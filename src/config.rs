use meru_interface::{Color, File};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, JsonSchema, Serialize, Deserialize)]
pub struct Config {
    pub model: Model,
    pub boot_rom: BootRom,
    pub custom_boot_roms: CustomBootRoms,
    pub palette: PaletteSelect,
    pub custom_palette: Palette,
    pub color_correction: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model: Model::Auto,
            boot_rom: BootRom::Internal,
            custom_boot_roms: CustomBootRoms::default(),
            palette: PaletteSelect::Pocket,
            custom_palette: PALETTE_GRAYSCALE,
            color_correction: true,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, JsonSchema, Serialize, Deserialize)]
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

#[derive(Clone, PartialEq, Eq, JsonSchema, Serialize, Deserialize)]
pub enum BootRom {
    None,
    Internal,
    Custom,
}

#[derive(Clone, Default, JsonSchema, Serialize, Deserialize)]
pub struct CustomBootRoms {
    pub dmg: Option<File>,
    pub cgb: Option<File>,
    // pub sgb: Option<File>,
    // pub sgb2: Option<File>,
    // pub agb: Option<File>,
}

#[rustfmt::skip]
const BOOT_ROMS: &[(&str, &[u8])] = &[
    ("DMG", include_bytes!("../assets/sameboy-bootroms/dmg_boot.bin")),
    ("CGB", include_bytes!("../assets/sameboy-bootroms/cgb_boot.bin")),
    ("SGB", include_bytes!("../assets/sameboy-bootroms/sgb_boot.bin")),
    ("SGB2",include_bytes!("../assets/sameboy-bootroms/sgb2_boot.bin")),
    ("AGB", include_bytes!("../assets/sameboy-bootroms/agb_boot.bin")),
];

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
            Model::Dmg => self.dmg.as_deref(),
            Model::Cgb => self.cgb.as_deref(),
            Model::Sgb => self.sgb.as_deref(),
            Model::Sgb2 => self.sgb2.as_deref(),
            Model::Agb => self.agb.as_deref(),
            Model::Auto => panic!(),
        }
    }
}

impl Config {
    pub fn boot_roms(&self) -> Result<BootRoms, std::io::Error> {
        Ok(match self.boot_rom {
            BootRom::None => BootRoms::default(),
            BootRom::Internal => {
                let lookup = |name: &str| {
                    BOOT_ROMS
                        .iter()
                        .find(|(n, _)| *n == name)
                        .map(|(_, b)| b.to_vec())
                };
                BootRoms {
                    dmg: lookup("DMG"),
                    cgb: lookup("CGB"),
                    sgb: lookup("SGB"),
                    sgb2: lookup("SGB2"),
                    agb: lookup("AGB"),
                }
            }
            BootRom::Custom => {
                let load = |file: &Option<File>| -> Result<Option<Vec<u8>>, std::io::Error> {
                    file.as_ref().map(|r| r.data()).transpose()
                };
                BootRoms {
                    dmg: load(&self.custom_boot_roms.dmg)?,
                    cgb: load(&self.custom_boot_roms.cgb)?,
                    sgb: None,
                    sgb2: None,
                    agb: None,
                    // sgb: load(&self.custom_boot_roms.sgb),
                    // sgb2: load(&self.custom_boot_roms.sgb2),
                    // agb: load(&self.custom_boot_roms.agb),
                }
            }
        })
    }

    pub fn palette(&self) -> &Palette {
        self.palette.get_palette().unwrap_or(&self.custom_palette)
    }
}

#[derive(Clone, PartialEq, Eq, JsonSchema, Serialize, Deserialize)]
pub enum PaletteSelect {
    Dmg,
    Pocket,
    Light,
    Grayscale,
    Custom,
}

pub type Palette = [Color; 4];

impl PaletteSelect {
    pub fn get_palette(&self) -> Option<&Palette> {
        Some(match self {
            PaletteSelect::Dmg => &PALETTE_DMG,
            PaletteSelect::Pocket => &PALETTE_POCKET,
            PaletteSelect::Light => &PALETTE_LIGHT,
            PaletteSelect::Grayscale => &PALETTE_GRAYSCALE,
            PaletteSelect::Custom => None?,
        })
    }
}

pub const PALETTE_DMG: Palette = [
    Color::new(120, 128, 16),
    Color::new(92, 120, 64),
    Color::new(56, 88, 76),
    Color::new(40, 64, 56),
];

pub const PALETTE_POCKET: Palette = [
    Color::new(200, 200, 168),
    Color::new(164, 164, 140),
    Color::new(104, 104, 84),
    Color::new(40, 40, 20),
];

pub const PALETTE_LIGHT: Palette = [
    Color::new(0, 178, 132),
    Color::new(0, 156, 116),
    Color::new(0, 104, 74),
    Color::new(0, 80, 56),
];

pub const PALETTE_GRAYSCALE: Palette = [
    Color::new(255, 255, 255),
    Color::new(170, 170, 170),
    Color::new(85, 85, 85),
    Color::new(0, 0, 0),
];
