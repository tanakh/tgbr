use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::{
    apu::Apu,
    bus,
    config::{Config, Model},
    consts::{SCREEN_HEIGHT, SCREEN_WIDTH},
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
    ppu: Ref<Ppu>,
    apu: Ref<Apu>,
    ctx: Context,
    model: Model,
    #[serde(skip)]
    rom: Ref<Rom>,
    #[serde(skip)]
    frame_buffer: Ref<FrameBuffer>,
    #[serde(skip)]
    audio_buffer: Ref<AudioBuffer>,
}

#[derive(Serialize)]
struct Context {
    bus: Ref<bus::Bus>,
    interrupt_enable: Ref<u8>,
    interrupt_flag: Ref<u8>,
    bus_context: BusContext,
}

#[derive(Serialize)]
struct BusContext {
    vram: Ref<Vec<u8>>,
    vram_lock: Ref<bool>,
    oam: Ref<Vec<u8>>,
    oam_lock: Ref<bool>,
}

impl GameBoy {
    pub fn new(rom: Rom, backup_ram: Option<Vec<u8>>, config: &Config) -> Result<Self> {
        let rom = Ref::new(rom);
        let mbc = create_mbc(&rom, backup_ram);
        let frame_buffer = Ref::new(FrameBuffer::new(
            SCREEN_WIDTH as usize,
            SCREEN_HEIGHT as usize,
        ));
        let audio_buffer = Ref::new(AudioBuffer::new());

        let interrupt_enable = Ref::new(0x00);
        let interrupt_flag = Ref::new(0x00);

        let vram = Ref::new(vec![0; 0x2000]);
        let vram_lock = Ref::new(false);
        let oam = Ref::new(vec![0; 0xA0]);
        let oam_lock = Ref::new(false);

        let ppu = Ref::new(Ppu::new(
            &vram,
            &oam,
            &oam_lock,
            &interrupt_flag,
            &frame_buffer,
            &config.dmg_palette,
        ));
        let apu = Ref::new(Apu::new(&audio_buffer));

        let io = Io::new(&ppu, &apu, &interrupt_enable, &interrupt_flag);
        let bus = Ref::new(bus::Bus::new(mbc, &config.boot_rom, io));
        let cpu = Cpu::new();

        // Set up the contents of registers after internal ROM execution
        let model = match rom.borrow().cgb_flag {
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
            ppu,
            apu,
            rom,
            ctx: Context {
                bus,
                interrupt_enable,
                interrupt_flag,
                bus_context: BusContext {
                    vram,
                    vram_lock,
                    oam,
                    oam_lock,
                },
            },
            model,
            frame_buffer,
            audio_buffer,
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
        self.audio_buffer.borrow_mut().buf.clear();

        let start_frame = self.ppu.borrow().frame();
        while start_frame == self.ppu.borrow().frame() {
            self.cpu.step(&mut self.ctx);
        }
    }

    pub fn rom(&self) -> &Ref<Rom> {
        &self.rom
    }

    pub fn set_dmg_palette(&mut self, palette: &[Color; 4]) {
        self.ppu.borrow_mut().set_dmg_palette(palette);
    }

    pub fn set_input(&mut self, input: &Input) {
        self.ctx.bus.borrow_mut().io().set_input(input);
    }

    pub fn frame_buffer(&self) -> &Ref<FrameBuffer> {
        &self.frame_buffer
    }

    pub fn audio_buffer(&self) -> &Ref<AudioBuffer> {
        &self.audio_buffer
    }

    pub fn backup_ram(&self) -> Option<Vec<u8>> {
        self.ctx
            .bus
            .borrow_mut()
            .mbc()
            .backup_ram()
            .map(|r| r.to_owned())
    }

    pub fn set_link_cable(&mut self, link_cable: Option<impl LinkCable + 'static>) {
        // FIXME: How to do this simpler?
        fn wrap_link_cable(link_cable: impl LinkCable + 'static) -> Ref<dyn LinkCable> {
            Ref(std::rc::Rc::new(std::cell::RefCell::new(link_cable)))
        }
        let link_cable = link_cable.map(wrap_link_cable);
        self.ctx.bus.borrow_mut().io().set_link_cable(link_cable);
    }

    // pub fn save_state() -> Value {
    //     todo!()
    // }
}

impl context::InterruptFlag for Context {
    fn interrupt_enable(&mut self) -> u8 {
        *self.interrupt_enable.borrow()
    }

    fn set_interrupt_enable(&mut self, data: u8) {
        *self.interrupt_enable.borrow_mut() = data;
    }

    fn interrupt_flag(&mut self) -> u8 {
        *self.interrupt_flag.borrow()
    }

    fn set_interrupt_flag(&mut self, data: u8) {
        *self.interrupt_flag.borrow_mut() = data;
    }
}

impl context::Bus for Context {
    fn tick(&mut self) {
        self.bus.borrow_mut().tick(&mut self.bus_context);
        self.bus.borrow_mut().io().tick();
    }

    fn read(&mut self, addr: u16) -> u8 {
        self.bus.borrow_mut().read(&mut self.bus_context, addr)
    }

    fn read_immutable(&mut self, addr: u16) -> Option<u8> {
        self.bus
            .borrow_mut()
            .read_immutable(&mut self.bus_context, addr)
    }

    fn write(&mut self, addr: u16, data: u8) {
        self.bus
            .borrow_mut()
            .write(&mut self.bus_context, addr, data)
    }
}

impl context::Vram for BusContext {
    fn read_vram(&self, addr: u16) -> u8 {
        if !*self.vram_lock.borrow() {
            self.vram.borrow()[addr as usize]
        } else {
            !0
        }
    }

    fn write_vram(&mut self, addr: u16, data: u8) {
        if !*self.vram_lock.borrow() {
            self.vram.borrow_mut()[addr as usize] = data;
        }
    }

    fn lock_vram(&mut self, lock: bool) {
        *self.vram_lock.borrow_mut() = lock;
    }
}

impl context::Oam for BusContext {
    fn read_oam(&self, addr: u8) -> u8 {
        if !*self.oam_lock.borrow() {
            self.oam.borrow()[addr as usize]
        } else {
            !0
        }
    }

    fn write_oam(&mut self, addr: u8, data: u8) {
        if !*self.oam_lock.borrow() {
            self.oam.borrow_mut()[addr as usize] = data;
        }
    }

    fn lock_oam(&mut self, lock: bool) {
        *self.oam_lock.borrow_mut() = lock;
    }
}
