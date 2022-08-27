use meru_interface::{
    AudioBuffer, Color, CoreInfo, EmulatorCore, FrameBuffer, InputData, KeyConfig,
};

use crate::{
    config::{Config, Model},
    consts,
    context::{self, Context},
    interface::LinkCable,
    io::Input,
    rom::{CgbFlag, Mbc, Rom, RomError},
};

pub struct GameBoy {
    rom_hash: [u8; 32],
    config: Config,
    corrected_frame_buffer: FrameBuffer,
    ctx: context::Context,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}")]
    RomError(#[from] RomError),
    #[error("This ROM support only CGB")]
    DoesNotSupportCgb,
    #[error("ROM hash mismatch")]
    RomHashMismatch,
    #[error("{0} is currently unsupported")]
    UnsupportedMbc(Mbc),
    #[error("deserialize failed: {0}")]
    DeserializeFailed(#[from] bincode::Error),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

const CORE_INFO: CoreInfo = CoreInfo {
    system_name: "Game Boy (TGBR)",
    abbrev: "gb",
    file_extensions: &["gb", "gbc"],
};

fn default_key_config() -> KeyConfig {
    use meru_interface::key_assign::*;

    #[rustfmt::skip]
    let keys = vec![
        ("Up", any!(keycode!(Up), pad_button!(0, DPadUp))),
        ("Down", any!(keycode!(Down), pad_button!(0, DPadDown))),
        ("Left", any!(keycode!(Left), pad_button!(0, DPadLeft))),
        ("Right", any!(keycode!(Right), pad_button!(0, DPadRight))),
        ("A", any!(keycode!(X), pad_button!(0, East))),
        ("B", any!(keycode!(Z), pad_button!(0, South))),
        ("Start", any!(keycode!(Return), pad_button!(0, Start))),
        ("Select", any!(keycode!(RShift), pad_button!(0, Select))),
    ];

    KeyConfig {
        controllers: vec![keys.into_iter().map(|(k, v)| (k.to_string(), v)).collect()],
    }
}

impl EmulatorCore for GameBoy {
    type Error = Error;
    type Config = Config;

    fn core_info() -> &'static CoreInfo {
        &CORE_INFO
    }

    fn try_from_file(
        data: &[u8],
        backup: Option<&[u8]>,
        config: &Self::Config,
    ) -> Result<Self, Self::Error>
    where
        Self: Sized,
    {
        let rom = Rom::from_bytes(data)?;

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
                    Err(Error::DoesNotSupportCgb)?
                } else {
                    Model::Cgb
                }
            }
        };

        log::info!("Model: {model:?}");

        let boot_rom = config.boot_roms()?.get(model).map(|r| r.to_owned());
        let backup = backup.map(|r| r.to_vec());
        let dmg_palette = config.palette();

        let mut ret = Self {
            rom_hash,
            config: config.clone(),
            corrected_frame_buffer: FrameBuffer::new(
                consts::SCREEN_WIDTH as _,
                consts::SCREEN_HEIGHT as _,
            ),
            ctx: Context::new(model, rom, &boot_rom, backup, dmg_palette)?,
        };

        if boot_rom.is_none() {
            // Do not use boot ROM
            // Set the values of the state after the boot ROM
            ret.setup_initial_state();
        }

        Ok(ret)
    }

    fn game_info(&self) -> Vec<(String, String)> {
        self.ctx
            .inner
            .inner
            .rom
            .info()
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect()
    }

    fn set_config(&mut self, config: &Self::Config) {
        use context::Ppu;
        self.config = config.clone();
        self.ctx.ppu_mut().set_dmg_palette(config.palette());
    }

    fn exec_frame(&mut self, render_graphics: bool) {
        use context::*;

        let mut audio_buffer = self.ctx.apu_mut().audio_buffer_mut();
        audio_buffer.samples.clear();
        audio_buffer.sample_rate = consts::AUDIO_SAMPLE_PER_FRAME as u32 * 60;

        self.ctx
            .ppu_mut()
            .frame_buffer_mut()
            .resize(consts::SCREEN_WIDTH as _, consts::SCREEN_HEIGHT as _);

        self.ctx.ppu_mut().set_render_graphics(render_graphics);

        let start_frame = self.ctx.ppu().frame();
        while start_frame == self.ctx.ppu().frame() {
            self.ctx.cpu.step(&mut self.ctx.inner);
        }

        if render_graphics {
            let cc =
                make_color_correction(self.ctx.model().is_cgb() && self.config.color_correction);
            cc.convert_frame_buffer(
                &mut self.corrected_frame_buffer,
                self.ctx.ppu_mut().frame_buffer_mut(),
            );
        }
    }

    fn reset(&mut self) {
        use context::*;

        let model = self.ctx.model();
        let backup_ram = self.backup();
        let mut rom = crate::rom::Rom::default();
        std::mem::swap(&mut rom, self.ctx.rom_mut());

        let boot_rom = self.ctx.inner.bus.boot_rom().clone();
        let dmg_palette = self.ctx.ppu().dmg_palette();

        self.ctx = Context::new(model, rom, &boot_rom, backup_ram, dmg_palette).unwrap();

        if boot_rom.is_none() {
            self.setup_initial_state();
        }
    }

    fn frame_buffer(&self) -> &FrameBuffer {
        &self.corrected_frame_buffer
    }
    fn audio_buffer(&self) -> &AudioBuffer {
        use context::Apu;
        self.ctx.apu().audio_buffer()
    }

    fn default_key_config() -> KeyConfig {
        default_key_config()
    }

    fn set_input(&mut self, input: &InputData) {
        let mut gb_input = Input::default();

        for (key, value) in &input.controllers[0] {
            match key.as_str() {
                "Up" => gb_input.up = *value,
                "Down" => gb_input.down = *value,
                "Left" => gb_input.left = *value,
                "Right" => gb_input.right = *value,
                "A" => gb_input.a = *value,
                "B" => gb_input.b = *value,
                "Start" => gb_input.start = *value,
                "Select" => gb_input.select = *value,
                _ => unreachable!(),
            }
        }

        let io = self.ctx.inner.bus.io();
        io.set_input(&mut self.ctx.inner.inner, &gb_input);
    }

    fn backup(&self) -> Option<Vec<u8>> {
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

    fn save_state(&self) -> Vec<u8> {
        let state = (&self.rom_hash, &self.ctx);
        bincode::serialize(&state).unwrap()
    }

    fn load_state(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        use context::*;
        // TODO: limitation: cannot restore connector

        // Deserialize object
        let (rom_hash, mut ctx): ([u8; 32], Context) = bincode::deserialize(data)?;

        // Restore unserialized fields
        if self.rom_hash != rom_hash {
            Err(Error::RomHashMismatch)?
        }

        std::mem::swap(self.ctx.rom_mut(), ctx.rom_mut());
        self.ctx = ctx;

        Ok(())
    }
}

fn make_color_correction(color_correction: bool) -> Box<dyn ColorCorrection> {
    if color_correction {
        Box::new(CorrectColor) as Box<dyn ColorCorrection>
    } else {
        Box::new(RawColor) as Box<dyn ColorCorrection>
    }
}

trait ColorCorrection {
    fn translate(&self, c: &Color) -> Color;

    fn convert_frame_buffer(&self, dest: &mut FrameBuffer, src: &FrameBuffer) {
        let width = src.width;
        let height = src.height;

        dest.resize(width, height);

        for y in 0..height {
            for x in 0..width {
                let c = self.translate(src.pixel(x, y));
                *dest.pixel_mut(x, y) = c;
            }
        }
    }
}

struct RawColor;

impl ColorCorrection for RawColor {
    fn translate(&self, c: &Color) -> Color {
        c.clone()
    }
}

struct CorrectColor;

impl ColorCorrection for CorrectColor {
    fn translate(&self, c: &Color) -> Color {
        let r = c.r as u16;
        let g = c.g as u16;
        let b = c.b as u16;
        Color::new(
            (((r * 26 + g * 4 + b * 2) / 32) as u8).min(240),
            (((g * 24 + b * 8) / 32) as u8).min(240),
            (((r * 6 + g * 4 + b * 22) / 32) as u8).min(240),
        )
    }
}

impl GameBoy {
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

    pub fn set_link_cable(&mut self, link_cable: Option<impl LinkCable + Send + Sync + 'static>) {
        let link_cable = link_cable.map(|r| Box::new(r) as Box<dyn LinkCable + Send + Sync>);
        self.ctx.inner.bus.io().set_link_cable(link_cable);
    }
}
