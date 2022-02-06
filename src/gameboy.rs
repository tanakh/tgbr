use anyhow::{bail, Result};

use crate::{
    apu::Apu,
    bus::Bus,
    config::{Config, Model},
    consts::{SCREEN_HEIGHT, SCREEN_WIDTH},
    cpu::Cpu,
    interface::{AudioBuffer, Color, FrameBuffer, Input},
    io::Io,
    mbc::create_mbc,
    ppu::Ppu,
    rom::{CgbFlag, Rom},
    util::Ref,
};

pub struct GameBoy {
    cpu: Cpu,
    io: Ref<Io>,
    ppu: Ref<Ppu>,
    apu: Ref<Apu>,
    rom: Ref<Rom>,
    frame_buffer: Ref<FrameBuffer>,
    audio_buffer: Ref<AudioBuffer>,
}

impl GameBoy {
    pub fn new(rom: Rom, config: &Config) -> Result<Self> {
        let rom = Ref::new(rom);
        let mbc = create_mbc(&rom);
        let frame_buffer = Ref::new(FrameBuffer::new(
            SCREEN_WIDTH as usize,
            SCREEN_HEIGHT as usize,
        ));
        let audio_buffer = Ref::new(AudioBuffer::new());

        let interrupt_enable = Ref::new(0x00);
        let interrupt_flag = Ref::new(0x00);

        let vram = Ref::new(vec![0; 0x2000]);
        let oam = Ref::new(vec![0; 0xA0]);

        let ppu = Ref::new(Ppu::new(
            &vram,
            &oam,
            &interrupt_flag,
            &frame_buffer,
            &config.dmg_palette,
        ));
        let apu = Ref::new(Apu::new(&audio_buffer));

        let io = Ref::new(Io::new(&ppu, &apu, &interrupt_enable, &interrupt_flag));

        let bus = Ref::new(Bus::new(&mbc, &vram, &oam, &io));
        let mut cpu = Cpu::new(&bus, &interrupt_enable, &interrupt_flag);

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

        // Set up the contents of registers after internal ROM execution
        let reg = cpu.register();

        match model {
            Model::Dmg => {
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

        Ok(Self {
            cpu,
            io,
            ppu,
            apu,
            rom,
            frame_buffer,
            audio_buffer,
        })
    }

    pub fn exec_frame(&mut self) {
        self.audio_buffer.borrow_mut().buf.clear();

        let start_frame = self.ppu.borrow().frame();
        while start_frame == self.ppu.borrow().frame() {
            self.cpu.step();
        }
    }

    pub fn set_dmg_palette(&mut self, palette: &[Color; 4]) {
        self.ppu.borrow_mut().set_dmg_palette(palette);
    }

    pub fn set_input(&mut self, input: &Input) {
        self.io.borrow_mut().set_input(input);
    }

    pub fn frame_buffer(&self) -> &Ref<FrameBuffer> {
        &self.frame_buffer
    }

    pub fn audio_buffer(&self) -> &Ref<AudioBuffer> {
        &self.audio_buffer
    }
}
