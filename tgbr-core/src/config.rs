use serde::{Deserialize, Serialize};

use crate::interface::Color;

const DEFAULT_DMG_PALETTE: [Color; 4] = [
    Color::new(255, 255, 255),
    Color::new(170, 170, 170),
    Color::new(85, 85, 85),
    Color::new(0, 0, 0),
];

pub struct Config {
    pub model: Model,
    pub dmg_palette: [Color; 4],
    pub boot_roms: BootRoms,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Model {
    Auto,
    Dmg,
    Sgb,
    Sgb2,
    Cgb,
    Agb,
}

impl Model {
    pub fn is_cgb(&self) -> bool {
        match self {
            Model::Cgb | Model::Agb => true,
            Model::Sgb | Model::Sgb2 | Model::Dmg => false,
            Model::Auto => panic!(),
        }
    }
}

#[derive(Default, Clone)]
pub struct BootRoms {
    pub dmg: Option<Vec<u8>>,
    pub cgb: Option<Vec<u8>>,
    pub sgb: Option<Vec<u8>>,
    pub sgb2: Option<Vec<u8>>,
    pub agb: Option<Vec<u8>>,
}

impl BootRoms {
    pub fn get(&self, model: Model) -> Option<&[u8]> {
        match model {
            Model::Dmg => self.dmg.as_ref().map(|r| r.as_slice()),
            Model::Cgb => self.cgb.as_ref().map(|r| r.as_slice()),
            Model::Sgb => self.sgb.as_ref().map(|r| r.as_slice()),
            Model::Sgb2 => self.sgb2.as_ref().map(|r| r.as_slice()),
            Model::Agb => self.agb.as_ref().map(|r| r.as_slice()),
            Model::Auto => panic!(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model: Model::Auto,
            dmg_palette: DEFAULT_DMG_PALETTE,
            boot_roms: Default::default(),
        }
    }
}

impl Config {
    pub fn set_model(mut self, model: Model) -> Self {
        self.model = model;
        self
    }
    pub fn set_dmg_palette(mut self, palette: &[Color; 4]) -> Self {
        self.dmg_palette = palette.clone();
        self
    }
    pub fn set_boot_rom(mut self, boot_roms: BootRoms) -> Self {
        self.boot_roms = boot_roms;
        self
    }
}
