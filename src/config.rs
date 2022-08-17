use meru_interface::{ConfigUi, Pixel, Ui};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub model: Model,
    pub boot_rom: BootRom,
    pub custom_boot_roms: CustomBootRoms,
    pub palette: PaletteSelect,
    pub color_correction: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model: Model::Auto,
            boot_rom: BootRom::Internal,
            custom_boot_roms: CustomBootRoms::default(),
            palette: PaletteSelect::Pocket,
            color_correction: true,
        }
    }
}

impl ConfigUi for Config {
    fn ui(&mut self, ui: &mut impl Ui) {
        ui.horizontal(|ui| {
            ui.label("Model:");
            ui.radio(
                &mut self.model,
                &[
                    ("Auto", Model::Auto),
                    ("CGB", Model::Cgb),
                    ("SGB", Model::Sgb),
                ],
            );
        });

        ui.label("Boot ROM:");
        ui.horizontal(|ui| {
            ui.radio(
                &mut self.boot_rom,
                &[
                    ("Do not use", BootRom::None),
                    ("Use internal ROM", BootRom::Internal),
                    ("Use specified ROM", BootRom::Custom),
                ],
            );
        });

        ui.enabled(self.boot_rom == BootRom::Custom, |ui| {
            ui.file(
                "DMG boot ROM:",
                &mut self.custom_boot_roms.dmg,
                &[("Boot ROM file", &["*"])],
            );

            ui.file(
                "CGB boot ROM:",
                &mut self.custom_boot_roms.cgb,
                &[("Boot ROM file", &["*"])],
            );
        });

        ui.label("Graphics:");
        ui.checkbox(&mut self.color_correction, "Color Correction");

        ui.label("GameBoy Palette:");

        ui.horizontal(|ui| {
            #[derive(Clone)]
            struct Palette(PaletteSelect);

            impl PartialEq for Palette {
                fn eq(&self, other: &Self) -> bool {
                    use PaletteSelect::*;
                    match (&self.0, &other.0) {
                        (Custom(_), Custom(_)) => true,
                        _ => self.0 == other.0,
                    }
                }
            }

            let mut palette = Palette(self.palette.clone());

            ui.combo_box(
                &mut palette,
                &[
                    ("GameBoy", Palette(PaletteSelect::Dmg)),
                    ("GameBoy Pocket", Palette(PaletteSelect::Pocket)),
                    ("GameBoy Light", Palette(PaletteSelect::Light)),
                    ("Grayscale", Palette(PaletteSelect::Grayscale)),
                    (
                        "Custom",
                        Palette(PaletteSelect::Custom(self.palette.get_palette().clone())),
                    ),
                ],
            );

            self.palette = palette.0;

            let cols = self.palette.get_palette().clone();

            for i in (0..4).rev() {
                let mut col = Pixel::new(cols[i].r, cols[i].g, cols[i].b);

                ui.color(&mut col);

                if let PaletteSelect::Custom(r) = &mut self.palette {
                    r[i] = Pixel::new(col.r, col.g, col.b);
                }
            }
        });
    }
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

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BootRom {
    None,
    Internal,
    Custom,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct CustomBootRoms {
    pub dmg: Option<PathBuf>,
    pub cgb: Option<PathBuf>,
    // pub sgb: Option<PathBuf>,
    // pub sgb2: Option<PathBuf>,
    // pub agb: Option<PathBuf>,
}

#[rustfmt::skip]
const BOOT_ROMS: &[(&str, &[u8])] = &[
    ("DMG", include_bytes!("../../assets/sameboy-bootroms/dmg_boot.bin")),
    ("CGB", include_bytes!("../../assets/sameboy-bootroms/cgb_boot.bin")),
    ("SGB", include_bytes!("../../assets/sameboy-bootroms/sgb_boot.bin")),
    ("SGB2",include_bytes!("../../assets/sameboy-bootroms/sgb2_boot.bin")),
    ("AGB", include_bytes!("../../assets/sameboy-bootroms/agb_boot.bin")),
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
                let load = |path: &Option<PathBuf>| {
                    path.as_ref().map(|path| std::fs::read(path)).transpose()
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
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaletteSelect {
    Dmg,
    Pocket,
    Light,
    Grayscale,
    Custom(Palette),
}

pub type Palette = [Pixel; 4];

impl PaletteSelect {
    pub fn get_palette(&self) -> &Palette {
        match self {
            PaletteSelect::Dmg => &PALETTE_DMG,
            PaletteSelect::Pocket => &PALETTE_POCKET,
            PaletteSelect::Light => &PALETTE_LIGHT,
            PaletteSelect::Grayscale => &PALETTE_GRAYSCALE,
            PaletteSelect::Custom(pal) => pal,
        }
    }
}

pub const PALETTE_DMG: Palette = [
    Pixel::new(120, 128, 16),
    Pixel::new(92, 120, 64),
    Pixel::new(56, 88, 76),
    Pixel::new(40, 64, 56),
];

pub const PALETTE_POCKET: Palette = [
    Pixel::new(200, 200, 168),
    Pixel::new(164, 164, 140),
    Pixel::new(104, 104, 84),
    Pixel::new(40, 40, 20),
];

pub const PALETTE_LIGHT: Palette = [
    Pixel::new(0, 178, 132),
    Pixel::new(0, 156, 116),
    Pixel::new(0, 104, 74),
    Pixel::new(0, 80, 56),
];

pub const PALETTE_GRAYSCALE: Palette = [
    Pixel::new(255, 255, 255),
    Pixel::new(170, 170, 170),
    Pixel::new(85, 85, 85),
    Pixel::new(0, 0, 0),
];
