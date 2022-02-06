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
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Model {
    Auto,
    Dmg,
    Cgb,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model: Model::Auto,
            dmg_palette: DEFAULT_DMG_PALETTE,
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
}
