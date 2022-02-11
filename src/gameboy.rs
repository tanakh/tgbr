use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::{
    apu::Apu,
    bus::Bus,
    config::{Config, Model},
    context,
    cpu::Cpu,
    interface::{AudioBuffer, Color, FrameBuffer, Input, LinkCable},
    io::Io,
    mbc::create_mbc,
    ppu::Ppu,
    rom::{CgbFlag, Rom},
    util::Ref,
};

#[derive(Serialize)]
pub struct GameBoy {
    cpu: Cpu,
    ctx: Context,
    model: Model,
    // FIXME: Remove this
    #[serde(skip)]
    frame_buffer: FrameBuffer,
}

#[derive(Serialize)]
struct Context {
    bus: Bus,
    bus_context: BusContext,
}

#[derive(Serialize)]
struct BusContext {
    #[serde(skip_serializing)]
    rom: Rom,
    ppu: Ref<Ppu>,
    ppu_context: PpuContext,
}

#[derive(Serialize)]
struct PpuContext {
    apu: Apu,
    vram: Vec<u8>,
    vram_lock: bool,
    oam: Vec<u8>,
    oam_lock: bool,
    interrupt_enable: u8,
    interrupt_flag: u8,
}

impl GameBoy {
    pub fn new(rom: Rom, backup_ram: Option<Vec<u8>>, config: &Config) -> Result<Self> {
        let mbc = create_mbc(&rom, backup_ram);

        let vram = vec![0; 0x2000];
        let oam = vec![0; 0xA0];

        let ppu = Ref::new(Ppu::new(&config.dmg_palette));
        let apu = Apu::new();

        let io = Io::new();
        let bus = Bus::new(mbc, &config.boot_rom, io);
        let cpu = Cpu::new();

        // Set up the contents of registers after internal ROM execution
        let model = match rom.cgb_flag {
            CgbFlag::NonCgb => {
                if config.model == Model::Auto {
                    Model::Dmg
                } else {
                    config.model
                }
            }
            CgbFlag::SupportCgb => {
                if config.model == Model::Auto {
                    Model::Cgb
                } else {
                    config.model
                }
            }
            CgbFlag::OnlyCgb => {
                if config.model == Model::Dmg {
                    bail!("This ROM support only CGB");
                } else {
                    Model::Cgb
                }
            }
        };

        let mut ret = Self {
            cpu,
            ctx: Context {
                bus,
                bus_context: BusContext {
                    rom,
                    ppu,
                    ppu_context: PpuContext {
                        apu,
                        vram,
                        vram_lock: false,
                        oam,
                        oam_lock: false,
                        interrupt_enable: 0,
                        interrupt_flag: 0,
                    },
                },
            },
            model,
            frame_buffer: Default::default(),
        };

        if !config.boot_rom.is_some() {
            // Do not use boot ROM
            // Set the values of the state after the boot ROM
            ret.setup_initial_state();
        }

        Ok(ret)
    }

    fn setup_initial_state(&mut self) {
        match self.model {
            Model::Dmg => {
                let reg = self.cpu.register();
                reg.a = 0x01;
                reg.f.unpack(0xB0);
                reg.b = 0x00;
                reg.c = 0x13;
                reg.d = 0x00;
                reg.e = 0xD8;
                reg.h = 0x01;
                reg.l = 0x4D;
                reg.sp = 0xFFFE;
                reg.pc = 0x0100;
            }
            Model::Cgb => {
                let reg = self.cpu.register();
                reg.a = 0x11;
                reg.f.unpack(0x80);
                reg.b = 0x00;
                reg.c = 0x00;
                reg.d = 0xFF;
                reg.e = 0x56;
                reg.h = 0x00;
                reg.l = 0x0D;
                reg.sp = 0xFFFE;
                reg.pc = 0x0100;
            }
            _ => unreachable!(),
        }
    }

    pub fn exec_frame(&mut self) {
        self.ctx
            .bus_context
            .ppu_context
            .apu
            .audio_buffer_mut()
            .buf
            .clear();

        let start_frame = self.ctx.bus_context.ppu.borrow().frame();
        while start_frame == self.ctx.bus_context.ppu.borrow().frame() {
            self.cpu.step(&mut self.ctx);
        }
    }

    // pub fn rom(&self) -> &Ref<Rom> {
    //     &self.rom
    // }

    pub fn set_dmg_palette(&mut self, palette: &[Color; 4]) {
        self.ctx
            .bus_context
            .ppu
            .borrow_mut()
            .set_dmg_palette(palette);
    }

    pub fn set_input(&mut self, input: &Input) {
        self.ctx
            .bus
            .io()
            .set_input(&mut self.ctx.bus_context, input);
    }

    pub fn frame_buffer(&mut self) -> &FrameBuffer {
        let ppu = self.ctx.bus_context.ppu.borrow();
        self.frame_buffer = ppu.frame_buffer().clone();
        &self.frame_buffer
    }

    pub fn audio_buffer(&self) -> &AudioBuffer {
        self.ctx.bus_context.ppu_context.apu.audio_buffer()
    }

    pub fn backup_ram(&mut self) -> Option<Vec<u8>> {
        self.ctx
            .bus
            .mbc()
            .backup_ram(&mut self.ctx.bus_context)
            .map(|r| r.to_owned())
    }

    pub fn set_link_cable(&mut self, link_cable: Option<impl LinkCable + 'static>) {
        let link_cable = link_cable.map(|r| Box::new(r) as Box<dyn LinkCable>);
        self.ctx.bus.io().set_link_cable(link_cable);
    }

    // pub fn save_state() -> Value {
    //     todo!()
    // }
}

impl context::Bus for Context {
    fn tick(&mut self) {
        self.bus.tick(&mut self.bus_context);
        for _ in 0..4 {
            self.bus_context
                .ppu
                .borrow_mut()
                .tick(&mut self.bus_context.ppu_context);
            self.bus_context.ppu_context.apu.tick();
        }
        self.bus.io().serial().tick(&mut self.bus_context);
        self.bus.io().tick(&mut self.bus_context);
    }

    fn read(&mut self, addr: u16) -> u8 {
        self.bus.read(&mut self.bus_context, addr)
    }

    fn read_immutable(&mut self, addr: u16) -> Option<u8> {
        self.bus.read_immutable(&mut self.bus_context, addr)
    }

    fn write(&mut self, addr: u16, data: u8) {
        self.bus.write(&mut self.bus_context, addr, data)
    }
}

impl context::Rom for BusContext {
    fn rom(&self) -> &Rom {
        &self.rom
    }
}

impl context::InterruptFlag for Context {
    fn interrupt_enable(&mut self) -> u8 {
        self.bus_context.interrupt_enable()
    }

    fn set_interrupt_enable(&mut self, data: u8) {
        self.bus_context.set_interrupt_enable(data)
    }

    fn interrupt_flag(&mut self) -> u8 {
        self.bus_context.interrupt_flag()
    }

    fn set_interrupt_flag(&mut self, data: u8) {
        self.bus_context.set_interrupt_flag(data)
    }
}

impl context::InterruptFlag for BusContext {
    fn interrupt_enable(&mut self) -> u8 {
        self.ppu_context.interrupt_enable()
    }

    fn set_interrupt_enable(&mut self, data: u8) {
        self.ppu_context.set_interrupt_enable(data)
    }

    fn interrupt_flag(&mut self) -> u8 {
        self.ppu_context.interrupt_flag()
    }

    fn set_interrupt_flag(&mut self, data: u8) {
        self.ppu_context.set_interrupt_flag(data)
    }
}

impl context::InterruptFlag for PpuContext {
    fn interrupt_enable(&mut self) -> u8 {
        self.interrupt_enable
    }

    fn set_interrupt_enable(&mut self, data: u8) {
        self.interrupt_enable = data;
    }

    fn interrupt_flag(&mut self) -> u8 {
        self.interrupt_flag
    }

    fn set_interrupt_flag(&mut self, data: u8) {
        self.interrupt_flag = data;
    }
}

impl context::Vram for BusContext {
    fn read_vram(&self, addr: u16, force: bool) -> u8 {
        self.ppu_context.read_vram(addr, force)
    }

    fn write_vram(&mut self, addr: u16, data: u8, force: bool) {
        self.ppu_context.write_vram(addr, data, force)
    }

    fn lock_vram(&mut self, lock: bool) {
        self.ppu_context.lock_vram(lock)
    }
}

impl context::Vram for PpuContext {
    fn read_vram(&self, addr: u16, force: bool) -> u8 {
        if force || !self.vram_lock {
            self.vram[addr as usize]
        } else {
            !0
        }
    }

    fn write_vram(&mut self, addr: u16, data: u8, force: bool) {
        if force || !self.vram_lock {
            self.vram[addr as usize] = data;
        }
    }

    fn lock_vram(&mut self, lock: bool) {
        self.vram_lock = lock;
    }
}

impl context::Oam for BusContext {
    fn read_oam(&self, addr: u8, force: bool) -> u8 {
        self.ppu_context.read_oam(addr, force)
    }

    fn write_oam(&mut self, addr: u8, data: u8, force: bool) {
        self.ppu_context.write_oam(addr, data, force)
    }

    fn lock_oam(&mut self, lock: bool) {
        self.ppu_context.lock_oam(lock)
    }
}

impl context::Oam for PpuContext {
    fn read_oam(&self, addr: u8, force: bool) -> u8 {
        if force || !self.oam_lock {
            self.oam[addr as usize]
        } else {
            !0
        }
    }

    fn write_oam(&mut self, addr: u8, data: u8, force: bool) {
        if force || !self.oam_lock {
            self.oam[addr as usize] = data;
        }
    }

    fn lock_oam(&mut self, lock: bool) {
        self.oam_lock = lock;
    }
}

impl context::Ppu for BusContext {
    fn read_ppu(&mut self, addr: u16) -> u8 {
        self.ppu.borrow_mut().read(addr)
    }

    fn write_ppu(&mut self, addr: u16, data: u8) {
        self.ppu.borrow_mut().write(addr, data);
    }
}

impl context::Apu for BusContext {
    fn read_apu(&mut self, addr: u16) -> u8 {
        self.ppu_context.apu.read(addr)
    }

    fn write_apu(&mut self, addr: u16, data: u8) {
        self.ppu_context.apu.write(addr, data)
    }
}
