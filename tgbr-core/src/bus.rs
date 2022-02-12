use log::{trace, warn};
use serde::{Deserialize, Serialize};

use crate::{
    context,
    io::Io,
    mbc::{Mbc, MbcTrait},
    util::trait_alias,
};

#[derive(Serialize, Deserialize)]
pub struct Bus {
    #[serde(with = "serde_bytes")]
    ram: Vec<u8>,
    #[serde(with = "serde_bytes")]
    hiram: Vec<u8>,
    #[serde(with = "serde_bytes")]
    boot_rom: Vec<u8>,
    map_boot_rom: bool,
    mbc: Mbc,
    io: Io,
    dma: Dma,
}

trait_alias!(pub trait Context = context::Rom + context::Ppu + context::Apu + context::InterruptFlag);

#[derive(Default, Serialize, Deserialize)]
struct Dma {
    source: u8,
    pos: u8,
    enabled: bool,
    delay: usize,
}

impl Bus {
    pub fn new(mbc: Mbc, boot_rom: &Option<Vec<u8>>, io: Io) -> Self {
        if let Some(boot_rom) = boot_rom {
            assert_eq!(boot_rom.len(), 0x100, "Boot ROM must be 256 bytes");
        }

        Self {
            ram: vec![0; 0x2000],
            hiram: vec![0; 0x7F],
            boot_rom: boot_rom
                .as_ref()
                .map_or_else(|| vec![0; 0x100], |r| r.clone()),
            map_boot_rom: boot_rom.is_some(),
            mbc,
            io,
            dma: Dma::default(),
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
            0x0000..=0x00FF => {
                if self.map_boot_rom {
                    self.boot_rom[addr as usize]
                } else {
                    self.mbc.read(ctx, addr)
                }
            }
            0x0100..=0x7fff => self.mbc.read(ctx, addr),
            0x8000..=0x9fff => ctx.ppu().read_vram(addr & 0x1fff),
            0xa000..=0xbfff => self.mbc.read(ctx, addr),
            0xc000..=0xfdff => self.ram[(addr & 0x1fff) as usize],
            0xfe00..=0xfe9f => {
                if !self.dma.enabled {
                    ctx.ppu().read_oam((addr & 0xff) as u8)
                } else {
                    !0
                }
            }
            0xfea0..=0xfeff => todo!("Read from Unusable address: ${addr:04x}"),
            0xff46 => self.dma.source, // DMA
            0xff50 => !0,              // BANK
            0xff00..=0xff7f => self.io.read(ctx, addr),
            0xff80..=0xfffe => self.hiram[(addr & 0x7f) as usize],
            0xffff => self.io.read(ctx, addr),
        }
    }

    pub fn write(&mut self, ctx: &mut impl Context, addr: u16, data: u8) {
        trace!("--> Write: ${addr:04X} = ${data:02X}");
        match addr {
            0x0000..=0x7fff => self.mbc.write(ctx, addr, data),
            0x8000..=0x9fff => ctx.ppu_mut().write_vram(addr & 0x1fff, data),
            0xa000..=0xbfff => self.mbc.write(ctx, addr, data),
            0xc000..=0xfdff => self.ram[(addr & 0x1fff) as usize] = data,
            0xfe00..=0xfe9f => {
                if !self.dma.enabled {
                    ctx.ppu_mut().write_oam((addr & 0xff) as u8, data, false)
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

    pub fn tick(&mut self, ctx: &mut impl Context) {
        self.process_dma(ctx);
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
        ctx.ppu_mut().write_oam(self.dma.pos, data, true);
        self.dma.pos += 1;
        if self.dma.pos == 0xA0 {
            self.dma.enabled = false;
        }
    }
}
