use std::{
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use directories::ProjectDirs;

use crate::input::{HotKeys, KeyConfig};

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

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub scaling: usize,
    pub palette: [tgbr_core::Color; 4],
    pub key_config: KeyConfig,
    pub hotkeys: HotKeys,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scaling: 4,
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
        fs::write(config_path()?, s)?;
        Ok(())
    }
}

fn config_path() -> Result<PathBuf> {
    let project_dir = ProjectDirs::from("", "", "tgbr")
        .ok_or_else(|| anyhow!("Cannot find project directory"))?;
    let config_dir = project_dir.config_dir();
    fs::create_dir_all(config_dir)?;
    Ok(config_dir.join("config.toml"))
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
        while self.recent.len() > 10 {
            self.recent.pop_back();
        }
    }
}

fn persistent_state_path() -> Result<PathBuf> {
    let project_dir = ProjectDirs::from("", "", "tgbr")
        .ok_or_else(|| anyhow!("Cannot find project directory"))?;
    let config_dir = project_dir.config_dir();
    fs::create_dir_all(config_dir)?;
    Ok(config_dir.join("state.toml"))
}

pub fn load_persistent_state() -> Result<PersistentState> {
    let ret = if let Ok(s) = std::fs::read_to_string(persistent_state_path()?) {
        serde_json::from_str(&s).map_err(|e| anyhow!("{}", e))?
    } else {
        Default::default()
    };
    Ok(ret)
}
