use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::{
    apu::Apu,
    bus::Bus,
    config::{Config, Model},
    context::{self, Context},
    cpu::Cpu,
    interface::{AudioBuffer, Color, FrameBuffer, Input, LinkCable},
    io::Io,
    mbc::create_mbc,
    ppu::Ppu,
    rom::{CgbFlag, Rom},
};

#[derive(Serialize, Deserialize)]
pub struct GameBoy {
    cpu: Cpu,
    rom_hash: [u8; 32],
    model: Model,
    #[serde(flatten)]
    ctx: context::Context,
}

impl GameBoy {
    pub fn new(rom: Rom, backup_ram: Option<Vec<u8>>, config: &Config) -> Result<Self> {
        let rom_hash = {
            use sha2::Digest;
            sha2::Sha256::digest(&rom.data).into()
        };

        let cpu = Cpu::new();
        let ppu = Ppu::new(&config.dmg_palette);
        let apu = Apu::new();
        let io = Io::new();
        let mbc = create_mbc(&rom, backup_ram);
        let bus = Bus::new(mbc, &config.boot_rom, io);

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
            rom_hash,
            model,
            ctx: Context::new(bus, rom, ppu, apu),
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
        use context::*;

        self.ctx.apu_mut().audio_buffer_mut().buf.clear();

        let start_frame = self.ctx.ppu().frame();
        while start_frame == self.ctx.ppu().frame() {
            self.cpu.step(&mut self.ctx);
        }
    }

    pub fn set_dmg_palette(&mut self, palette: &[Color; 4]) {
        use context::*;
        self.ctx.ppu_mut().set_dmg_palette(palette);
    }

    pub fn set_input(&mut self, input: &Input) {
        self.ctx.bus.io().set_input(&mut self.ctx.inner, input);
    }

    pub fn frame_buffer(&self) -> &FrameBuffer {
        use context::*;
        self.ctx.ppu().frame_buffer()
    }

    pub fn audio_buffer(&self) -> &AudioBuffer {
        use context::*;
        self.ctx.apu().audio_buffer()
    }

    pub fn backup_ram(&mut self) -> Option<Vec<u8>> {
        self.ctx
            .bus
            .mbc()
            .backup_ram(&mut self.ctx.inner)
            .map(|r| r.to_owned())
    }

    pub fn set_link_cable(&mut self, link_cable: Option<impl LinkCable + 'static>) {
        let link_cable = link_cable.map(|r| Box::new(r) as Box<dyn LinkCable>);
        self.ctx.bus.io().set_link_cable(link_cable);
    }

    pub fn save_state(&self) -> Vec<u8> {
        let mut ret = vec![];
        ciborium::ser::into_writer(self, &mut ret).unwrap();
        ret
    }

    pub fn load_state(&mut self, data: &[u8]) -> Result<()> {
        // TODO: limitation: cannot restore connector

        // Deserialize object
        let mut gb: GameBoy = ciborium::de::from_reader(data)?;

        // Restore unserialized fields
        if self.rom_hash != gb.rom_hash {
            bail!("ROM hash mismatch");
        }
        std::mem::swap(&mut self.ctx.inner.rom, &mut gb.ctx.inner.rom);

        *self = gb;
        Ok(())
    }
}
