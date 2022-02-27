use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::{
    config::{Config, Model},
    context::{self, Context},
    interface::{AudioBuffer, Color, FrameBuffer, Input, LinkCable},
    rom::{CgbFlag, Rom},
};

#[derive(Serialize, Deserialize)]
pub struct GameBoy {
    rom_hash: [u8; 32],
    #[serde(flatten)]
    ctx: context::Context,
}

impl GameBoy {
    pub fn new(rom: Rom, backup_ram: Option<Vec<u8>>, config: &Config) -> Result<Self> {
        let rom_hash = {
            use sha2::Digest;
            sha2::Sha256::digest(&rom.data).into()
        };

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

        log::info!("Model: {model:?}");

        let boot_rom = config.boot_roms.get(model).map(|r| r.to_owned());

        let mut ret = Self {
            rom_hash,
            ctx: Context::new(model, rom, &boot_rom, backup_ram, &config.dmg_palette),
        };

        if !boot_rom.is_some() {
            // Do not use boot ROM
            // Set the values of the state after the boot ROM
            ret.setup_initial_state();
        }

        Ok(ret)
    }

    fn setup_initial_state(&mut self) {
        match context::Model::model(&self.ctx) {
            Model::Dmg => {
                let reg = self.ctx.cpu.register();
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
                let reg = self.ctx.cpu.register();
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

    pub fn reset(&mut self) {
        use context::*;

        let model = self.ctx.model();
        let backup_ram = self.backup_ram();
        let mut rom = crate::rom::Rom::default();
        std::mem::swap(&mut rom, self.ctx.rom_mut());

        let boot_rom = self.ctx.inner.bus.boot_rom().clone();
        let dmg_palette = self.ctx.inner.inner.ppu.dmg_palette();

        self.ctx = Context::new(model, rom, &boot_rom, backup_ram, dmg_palette);

        if !boot_rom.is_some() {
            self.setup_initial_state();
        }
    }

    pub fn exec_frame(&mut self) {
        use context::*;

        self.ctx.apu_mut().audio_buffer_mut().buf.clear();

        let start_frame = self.ctx.inner.inner.ppu.frame();
        while start_frame == self.ctx.inner.inner.ppu.frame() {
            self.ctx.cpu.step(&mut self.ctx.inner);
        }
    }

    pub fn model(&self) -> Model {
        use context::Model;
        self.ctx.model()
    }

    pub fn set_dmg_palette(&mut self, palette: &[Color; 4]) {
        self.ctx.inner.inner.ppu.set_dmg_palette(palette);
    }

    pub fn set_input(&mut self, input: &Input) {
        let io = self.ctx.inner.bus.io();
        io.set_input(&mut self.ctx.inner.inner, input);
    }

    pub fn frame_buffer(&self) -> &FrameBuffer {
        self.ctx.inner.inner.ppu.frame_buffer()
    }

    pub fn audio_buffer(&self) -> &AudioBuffer {
        self.ctx.inner.inner.apu.audio_buffer()
    }

    pub fn backup_ram(&mut self) -> Option<Vec<u8>> {
        use crate::mbc::MbcTrait;
        let external_ram = self.ctx.backup_ram();
        let internal_ram = self.ctx.inner.bus.mbc().internal_ram();
        assert!(!(external_ram.is_some() && internal_ram.is_some()));
        if external_ram.is_some() {
            external_ram
        } else {
            internal_ram.map(|r| r.to_vec())
        }
    }

    pub fn set_link_cable(&mut self, link_cable: Option<impl LinkCable + Send + Sync + 'static>) {
        let link_cable = link_cable.map(|r| Box::new(r) as Box<dyn LinkCable + Send + Sync>);
        self.ctx.inner.bus.io().set_link_cable(link_cable);
    }

    pub fn save_state(&self) -> Vec<u8> {
        let mut ret = vec![];
        ciborium::ser::into_writer(self, &mut ret).unwrap();
        ret
    }

    pub fn load_state(&mut self, data: &[u8]) -> Result<()> {
        use context::*;
        // TODO: limitation: cannot restore connector

        // Deserialize object
        let mut gb: GameBoy = ciborium::de::from_reader(data)?;

        // Restore unserialized fields
        if self.rom_hash != gb.rom_hash {
            bail!("ROM hash mismatch");
        }

        std::mem::swap(self.ctx.rom_mut(), gb.ctx.rom_mut());
        *self = gb;

        Ok(())
    }
}
