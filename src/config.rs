use anyhow::{anyhow, Result};
use directories::ProjectDirs;
use log::info;
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
};

use crate::input::{HotKeys, KeyConfig};

const FRAME_SKIP_ON_TURBO: usize = 5;
const AUDIO_FREQUENCY: usize = 48000;
const AUDIO_BUFFER_SAMPLES: usize = 2048;

pub type Palette = [tgbr_core::Color; 4];

const PALETTE_PRESET: Palette = {
    use tgbr_core::Color;
    // [
    //     Color::new(155, 188, 15),
    //     Color::new(139, 172, 15),
    //     Color::new(48, 98, 48),
    //     Color::new(15, 56, 15),
    // ]

    // [
    //     Color::new(155, 188, 15),
    //     Color::new(136, 170, 10),
    //     Color::new(48, 98, 48),
    //     Color::new(15, 56, 15),
    // ]

    // [
    //     Color::new(160, 207, 10),
    //     Color::new(140, 191, 10),
    //     Color::new(46, 115, 32),
    //     Color::new(0, 63, 0),
    // ]

    [
        Color::new(200, 200, 168),
        Color::new(164, 164, 140),
        Color::new(104, 104, 84),
        Color::new(40, 40, 20),
    ]
};

#[rustfmt::skip]
const BOOT_ROMS: &[(&str, &[u8])] = &[
    ("SameBoy-DMG", include_bytes!("../assets/sameboy-bootroms/dmg_boot.bin")),
    ("SameBoy-CGB", include_bytes!("../assets/sameboy-bootroms/cgb_boot.bin")),
    ("SameBoy-SGB", include_bytes!("../assets/sameboy-bootroms/sgb_boot.bin")),
    ("SameBoy-SGB2",include_bytes!("../assets/sameboy-bootroms/sgb2_boot.bin")),
    ("SameBoy-CGB", include_bytes!("../assets/sameboy-bootroms/cgb_boot.bin")),
    ("SameBoy-AGB", include_bytes!("../assets/sameboy-bootroms/agb_boot.bin")),
];

#[derive(Serialize, Deserialize)]
pub struct Config {
    save_dir: PathBuf,
    state_dir: PathBuf,
    show_fps: bool,
    scaling: usize,
    auto_state_save_freq: usize,
    auto_state_save_limit: usize,
    boot_rom: Option<String>,
    palette: [tgbr_core::Color; 4],
    key_config: KeyConfig,
    hotkeys: HotKeys,
}

impl Default for Config {
    fn default() -> Self {
        let (save_dir, state_dir) = if let Ok(project_dirs) = project_dirs() {
            (
                project_dirs.data_dir().to_owned(),
                project_dirs
                    .state_dir()
                    .unwrap_or_else(|| project_dirs.data_dir())
                    .to_owned(),
            )
        } else {
            (PathBuf::from("save"), PathBuf::from("state"))
        };

        fs::create_dir_all(&save_dir).unwrap();
        fs::create_dir_all(&state_dir).unwrap();

        Self {
            save_dir,
            state_dir,
            show_fps: false,
            scaling: 4,
            auto_state_save_freq: 60,
            auto_state_save_limit: 10 * 60,
            boot_rom: Some("SameBoy-DMG".to_owned()),
            palette: PALETTE_PRESET,
            key_config: KeyConfig::default(),
            hotkeys: HotKeys::default(),
        }
    }
}

impl Drop for Config {
    fn drop(&mut self) {
        self.save().unwrap();
    }
}

impl Config {
    fn save(&self) -> Result<()> {
        let s = serde_json::to_string_pretty(self)?;
        let path = config_path()?;
        fs::write(&path, s)?;
        info!("Saved config file: {:?}", path.display());
        Ok(())
    }

    pub fn scaling(&self) -> usize {
        self.scaling
    }

    pub fn set_scaling(&mut self, scaling: usize) {
        self.scaling = scaling;
        self.save().unwrap();
    }

    pub fn save_dir(&self) -> &Path {
        &self.save_dir
    }

    pub fn set_save_dir(&mut self, save_dir: PathBuf) {
        self.save_dir = save_dir;
    }

    pub fn state_dir(&self) -> &PathBuf {
        &self.state_dir
    }

    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    pub fn set_palette(&mut self, palette: Palette) {
        self.palette = palette;
    }

    pub fn boot_rom(&self) -> Option<Vec<u8>> {
        self.boot_rom.as_ref().map(|r| {
            BOOT_ROMS
                .iter()
                .find(|(name, _)| name == &r)
                .unwrap()
                .1
                .to_vec()
        })
    }

    pub fn key_config(&self) -> &KeyConfig {
        &self.key_config
    }

    pub fn hotkeys(&self) -> &HotKeys {
        &self.hotkeys
    }

    pub fn auto_state_save_freq(&self) -> usize {
        self.auto_state_save_freq
    }

    pub fn auto_state_save_limit(&self) -> usize {
        self.auto_state_save_limit
    }
}

fn project_dirs() -> Result<ProjectDirs> {
    let ret = ProjectDirs::from("", "", "tgbr")
        .ok_or_else(|| anyhow!("Cannot find project directory"))?;
    Ok(ret)
}

fn config_path() -> Result<PathBuf> {
    let project_dirs = project_dirs()?;
    let config_dir = project_dirs.config_dir();
    fs::create_dir_all(config_dir)?;
    Ok(config_dir.join("config.json"))
}

pub fn load_config() -> Result<Config> {
    let ret = if let Ok(s) = std::fs::read_to_string(config_path()?) {
        serde_json::from_str(&s).map_err(|e| anyhow!("{}", e))?
    } else {
        Config::default()
    };
    Ok(ret)
}

#[derive(Default, Serialize, Deserialize)]
pub struct PersistentState {
    pub recent: VecDeque<PathBuf>,
}

impl Drop for PersistentState {
    fn drop(&mut self) {
        let s = serde_json::to_string_pretty(self).unwrap();
        fs::write(persistent_state_path().unwrap(), s).unwrap();
    }
}

impl PersistentState {
    pub fn add_recent(&mut self, path: impl AsRef<Path>) {
        let path = path.as_ref().to_owned();
        if self.recent.contains(&path) {
            self.recent.retain(|p| p != &path);
        }
        self.recent.push_front(path);
        while self.recent.len() > 20 {
            self.recent.pop_back();
        }
    }
}

fn persistent_state_path() -> Result<PathBuf> {
    let project_dirs = project_dirs()?;
    let config_dir = project_dirs.config_dir();
    fs::create_dir_all(config_dir)?;
    Ok(config_dir.join("state.json"))
}

pub fn load_persistent_state() -> Result<PersistentState> {
    let ret = if let Ok(s) = std::fs::read_to_string(persistent_state_path()?) {
        serde_json::from_str(&s).map_err(|e| anyhow!("{}", e))?
    } else {
        Default::default()
    };
    Ok(ret)
}
