use bitvec::prelude::*;
use log::{debug, info, trace, warn};
use serde::{Deserialize, Serialize};
use std::cmp::max;

use crate::{
    config, context,
    io::Io,
    mbc::{Mbc, MbcTrait},
    ppu,
    util::{pack, trait_alias},
};

#[derive(Serialize, Deserialize)]
pub struct Bus {
    #[serde(with = "serde_bytes")]
    ram: Vec<u8>,
    ram_bank: u8,
    #[serde(with = "serde_bytes")]
    hiram: Vec<u8>,
    #[serde(with = "serde_bytes")]
    boot_rom: Option<Vec<u8>>,
    map_boot_rom: bool,
    vram_bank: u8,
    current_speed: u8,
    prepare_speed_switch: u8,
    switch_delay: u16,
    mbc: Mbc,
    io: Io,
    dma: Dma,
    hdma: Hdma,
    // Undocumented registers
    reg_ff72: u8,
    reg_ff73: u8,
    reg_ff75: u8,
}

trait_alias!(pub trait Context =
    context::Rom + context::ExternalRam + context::Vram +
    context::Oam + context::Ppu + context::Apu + context::InterruptFlag + context::Model);

#[derive(Default, Serialize, Deserialize)]
struct Dma {
    source: u8,
    pos: u8,
    enabled: bool,
    delay: usize,
}

#[derive(Default, Serialize, Deserialize)]
struct Hdma {
    source: u16,
    dest: u16,
    mode: HdmaMode,
    length: u8,
    enabled_general_dma: bool,
    enabled_hblank_dma: bool,
    prev_hblank: bool,
}

#[derive(Serialize, Deserialize)]
enum HdmaMode {
    General,
    HBlank,
}

impl Default for HdmaMode {
    fn default() -> Self {
        HdmaMode::General
    }
}

impl Bus {
    pub fn new(model: config::Model, mbc: Mbc, boot_rom: &Option<Vec<u8>>, io: Io) -> Self {
        if let Some(boot_rom) = boot_rom {
            if !model.is_cgb() {
                assert_eq!(boot_rom.len(), 0x100, "DMG Boot ROM must be 256 bytes");
            } else {
                assert_eq!(boot_rom.len(), 0x900, "CGB Boot ROM must be 2304 bytes");
            }
        }

        let ram_size = if model.is_cgb() { 0x8000 } else { 0x2000 };

        Self {
            ram: vec![0; ram_size],
            ram_bank: 1,
            hiram: vec![0; 0x7F],
            boot_rom: boot_rom.clone(),
            map_boot_rom: boot_rom.is_some(),
            vram_bank: 0,
            current_speed: 0,
            prepare_speed_switch: 0,
            switch_delay: 0,
            mbc,
            io,
            dma: Dma::default(),
            hdma: Hdma::default(),
            reg_ff72: 0,
            reg_ff73: 0,
            reg_ff75: 0,
        }
    }

    pub fn read(&mut self, ctx: &mut impl Context, addr: u16) -> u8 {
        let data = self.read_(ctx, addr);
        trace!("<-- Read:  ${addr:04X} = ${data:02X}");
        data
    }

    pub fn read_immutable(&mut self, ctx: &mut impl Context, addr: u16) -> Option<u8> {
        match addr {
            0xff00..=0xff7f | 0xffff => None,
            _ => Some(self.read_(ctx, addr)),
        }
    }

    fn read_(&mut self, ctx: &mut impl Context, addr: u16) -> u8 {
        match addr {
            0x0100..=0x01ff => self.mbc.read(ctx, addr),
            0x0000..=0x08ff => {
                let is_boot_rom = self.map_boot_rom
                    && self
                        .boot_rom
                        .as_ref()
                        .map_or(false, |r| r.len() > addr as usize);
                if is_boot_rom {
                    if let Some(boot_rom) = &self.boot_rom {
                        boot_rom[addr as usize]
                    } else {
                        !0
                    }
                } else {
                    self.mbc.read(ctx, addr)
                }
            }
            0x0100..=0x7fff => self.mbc.read(ctx, addr),
            0x8000..=0x9fff => {
                ctx.vram()[((addr & 0x1fff) | (self.vram_bank as u16 * 0x2000)) as usize]
            }
            0xa000..=0xbfff => self.mbc.read(ctx, addr),
            0xc000..=0xfdff => {
                let addr = addr % 0x1fff;
                let bank = addr & 0x1000;
                self.ram[((addr & 0x0fff) + bank * self.ram_bank as u16) as usize]
            }
            0xfe00..=0xfe9f => {
                if !self.dma.enabled {
                    ctx.oam()[(addr & 0xff) as usize]
                } else {
                    !0
                }
            }
            0xfea0..=0xfeff => {
                warn!("Read from Unusable address: ${addr:04x}");
                !0
            }
            0xff46 => self.dma.source, // DMA
            0xff50 => !0,              // BANK

            0xff4c => !0, // KEY0 CPU mode register
            0xff4d => !0, // KEY1 - CGB Mode Only - Prepare Speed Switch
            0xff4e => !0, // ???

            // VBK - CGB Mode Only - VRAM Bank (R/W)
            0xff4f => {
                if ctx.model().is_cgb() {
                    pack!(0..=0 => self.vram_bank, 1..=7 => !0)
                } else {
                    !0
                }
            }

            0xff51 => !0, // HDMA1 (New DMA Source, High) (W) - CGB Mode Only
            0xff52 => !0, // HDMA2 (New DMA Source, Low) (W) - CGB Mode Only
            0xff53 => !0, // HDMA3 (New DMA Destination, High) (W) - CGB Mode Only
            0xff54 => !0, // HDMA4 (New DMA Destination, Low) (W) - CGB Mode Only

            // HDMA5 (New DMA Length/Mode/Start) (W) - CGB Mode Only
            0xff55 => {
                if ctx.model().is_cgb() {
                    pack! {
                        7 => !self.hdma.enabled_hblank_dma,
                        0..=6 => self.hdma.length,
                    }
                } else {
                    !0
                }
            }
            // SVBK - CGB Mode Only - WRAM Bank
            0xff70 => !0,

            // Undocumented registers
            0xff72 => self.reg_ff72,
            0xff73 => self.reg_ff73,
            0xff75 => pack! {
                7..=7 => !0,
                4..=6 => self.reg_ff75,
                0..=3 => !0,
            },

            0xff00..=0xff7f => self.io.read(ctx, addr),
            0xff80..=0xfffe => self.hiram[(addr & 0x7f) as usize],
            0xffff => self.io.read(ctx, addr),
        }
    }

    pub fn write(&mut self, ctx: &mut impl Context, addr: u16, data: u8) {
        trace!("--> Write: ${addr:04X} = ${data:02X}");
        match addr {
            0x0000..=0x7fff => self.mbc.write(ctx, addr, data),
            0x8000..=0x9fff => {
                ctx.vram_mut()[((addr & 0x1fff) | (self.vram_bank as u16 * 0x2000)) as usize] = data
            }
            0xa000..=0xbfff => self.mbc.write(ctx, addr, data),
            0xc000..=0xfdff => {
                let addr = addr % 0x1fff;
                let bank = addr & 0x1000;
                self.ram[((addr & 0x0fff) + bank * self.ram_bank as u16) as usize] = data;
            }
            0xfe00..=0xfe9f => {
                if !self.dma.enabled && !ctx.oam_lock() {
                    ctx.oam_mut()[(addr & 0xff) as usize] = data;
                }
            }
            0xfea0..=0xfeff => warn!("Write to Unusable address: ${addr:04X} = ${data:02X}"),

            0xff46 => {
                // DMA
                self.dma.source = data;
                self.dma.pos = 0;
                self.dma.enabled = false;
                self.dma.delay = 2;
            }
            0xff50 => self.map_boot_rom = data & 1 == 0, // BANK

            // KEY0 CPU mode register
            0xff4c => {
                if ctx.model().is_cgb() {
                    let mode = match data.view_bits::<Lsb0>()[2..=3].load::<u8>() {
                        0 => context::RunningMode::Cgb,
                        1 => context::RunningMode::Dmg,
                        2 => context::RunningMode::Pgb1,
                        3 => context::RunningMode::Pgb2,
                        _ => unreachable!(),
                    };

                    ctx.set_running_mode(mode);
                    debug!("KEY0: CGB mode changed: {mode:?}");
                } else {
                    warn!("KEY0 write on non-CGB");
                }
            }

            // KEY1 - CGB Mode Only - Prepare Speed Switch
            0xff4d => {
                if ctx.model().is_cgb() {
                    debug!("KEY1: {data:02X}");
                    self.prepare_speed_switch = data & 1;
                } else {
                    warn!("KEY1 write on non-CGB");
                }
            }

            // VBK - CGB Mode Only - VRAM Bank (R/W)
            0xff4f => {
                if ctx.model().is_cgb() {
                    self.vram_bank = data & 1;
                } else {
                    warn!("VBK write on non-CGB");
                }
            }
            // HDMA1 (New DMA Source, High) (W) - CGB Mode Only
            0xff51 => {
                if ctx.model().is_cgb() {
                    self.hdma.source.view_bits_mut::<Lsb0>()[8..=15].store(data);
                } else {
                    warn!("HDMA1 write on non-CGB");
                }
            }
            // HDMA2 (New DMA Source, Low) (W) - CGB Mode Only
            0xff52 => {
                if ctx.model().is_cgb() {
                    self.hdma.source.view_bits_mut::<Lsb0>()[0..=7].store(data & !0xf);
                } else {
                    warn!("HDMA2 write on non-CGB");
                }
            }
            // HDMA3 (New DMA Destination, High) (W) - CGB Mode Only
            0xff53 => {
                if ctx.model().is_cgb() {
                    self.hdma.dest.view_bits_mut::<Lsb0>()[8..=12].store(data & 0x1f);
                } else {
                    warn!("HDMA3 write on non-CGB");
                }
            }
            // HDMA4 (New DMA Destination, Low) (W) - CGB Mode Only
            0xff54 => {
                if ctx.model().is_cgb() {
                    self.hdma.dest.view_bits_mut::<Lsb0>()[0..=7].store(data & !0xf);
                } else {
                    warn!("HDMA4 write on non-CGB");
                }
            }
            // HDMA5 (New DMA Length/Mode/Start) (W) - CGB Mode Only
            0xff55 => {
                if ctx.model().is_cgb() {
                    let v = data.view_bits::<Lsb0>();

                    assert!(!self.hdma.enabled_general_dma);

                    if self.hdma.enabled_hblank_dma {
                        assert!(!v[7], "General DMA start on doing HBLANK DMA");
                        self.hdma.enabled_hblank_dma = false;
                    } else {
                        if v[7] {
                            // HBlank DMA
                            self.hdma.enabled_hblank_dma = true;
                            self.hdma.length = v[0..=6].load();
                        } else {
                            // General Purpose DMA
                            self.hdma.enabled_general_dma = true;
                            self.hdma.length = v[0..=6].load();
                        }
                    }
                } else {
                    warn!("HDMA5 write on non-CGB");
                }
            }

            // SVBK - CGB Mode Only - WRAM Bank
            0xff70 => {
                if ctx.model().is_cgb() {
                    self.ram_bank = max(1, data & 0x7);
                } else {
                    warn!("SVBK write on non-CGB");
                }
            }

            // Undocumented registers
            0xff72 => self.reg_ff72 = data,
            0xff73 => self.reg_ff73 = data,
            0xff75 => self.reg_ff75.view_bits_mut::<Lsb0>()[4..=6]
                .store(data.view_bits::<Lsb0>()[4..=6].load::<u8>()),

            0xff00..=0xff7f => self.io.write(ctx, addr, data),
            0xff80..=0xfffe => self.hiram[(addr & 0x7f) as usize] = data,
            0xffff => self.io.write(ctx, addr, data),
        };
    }

    pub fn io(&mut self) -> &mut Io {
        &mut self.io
    }

    pub fn mbc(&mut self) -> &mut Mbc {
        &mut self.mbc
    }

    pub fn boot_rom(&self) -> &Option<Vec<u8>> {
        &self.boot_rom
    }

    pub fn current_speed(&self) -> u8 {
        self.current_speed
    }

    pub fn stop(&mut self) {
        if self.prepare_speed_switch != 0 {
            self.switch_delay = 2050;
        }
    }

    pub fn tick(&mut self, ctx: &mut impl Context) {
        self.process_dma(ctx);
        self.process_hdma(ctx);

        if self.switch_delay > 0 {
            assert!(self.prepare_speed_switch != 0);

            self.switch_delay -= 1;
            if self.switch_delay == 0 {
                info!(
                    "Switch speed: {} -> {}",
                    self.current_speed,
                    self.current_speed ^ 1
                );
                self.current_speed ^= 1;
                self.prepare_speed_switch = 0;
                ctx.wake();
            }
        }
    }

    fn process_dma(&mut self, ctx: &mut impl Context) {
        if self.dma.delay > 0 {
            self.dma.delay -= 1;
            if self.dma.delay == 0 {
                self.dma.enabled = true;
            }
            return;
        }
        if !self.dma.enabled {
            return;
        }
        log::trace!(
            "<-> DMA:   ${:02X}{:02X} -> $FE{:02X}",
            self.dma.source,
            self.dma.pos,
            self.dma.pos
        );
        let data = self.read_(ctx, (self.dma.source as u16) << 8 | self.dma.pos as u16);
        ctx.oam_mut()[self.dma.pos as usize] = data;
        self.dma.pos += 1;
        if self.dma.pos == 0xA0 {
            self.dma.enabled = false;
        }
    }

    fn process_hdma(&mut self, ctx: &mut impl Context) {
        assert!(!(self.hdma.enabled_general_dma && self.hdma.enabled_hblank_dma));

        let cur_hblank = ctx.mode() == ppu::Mode::Hblank;
        let enter_hblank = !self.hdma.prev_hblank && cur_hblank;
        self.hdma.prev_hblank = cur_hblank;

        if self.hdma.enabled_general_dma || (self.hdma.enabled_hblank_dma && enter_hblank) {
            log::trace!("HDMA: ${:04X} -> ${:04X}", self.hdma.source, self.hdma.dest);
            for i in 0..16 {
                let dat = self.read_(ctx, self.hdma.source + i);
                self.write(ctx, 0x8000 | self.hdma.dest + i, dat);
            }
            self.hdma.source = self.hdma.source.wrapping_add(16);
            self.hdma.dest = self.hdma.dest.wrapping_add(16);
            if self.hdma.dest >= 0x2000 {
                warn!("HDMA destination overflow");
                self.hdma.dest = self.hdma.dest & 0x1fff;
            }

            let (new_length, ovf) = self.hdma.length.overflowing_sub(1);
            self.hdma.length = new_length;
            if ovf {
                self.hdma.enabled_general_dma = false;
                self.hdma.enabled_hblank_dma = false;
            }
            ctx.stall_cpu(if self.current_speed == 0 { 8 } else { 16 });
        }
    }
}
