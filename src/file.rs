use anyhow::{anyhow, bail, Result};
use log::info;
use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

use tgbr_core::Rom;

const SAVE_DIR: &str = "./save";
const STATE_DIR: &str = "./state";

fn atomic_write_file(file: &Path, data: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut f = tempfile::NamedTempFile::new()?;
    f.write_all(data)?;
    f.persist(file)?;
    Ok(())
}

fn get_save_file_path(rom_file: &Path) -> Result<PathBuf> {
    let sav_file = rom_file
        .file_stem()
        .ok_or_else(|| anyhow!("Invalid file name: {}", rom_file.display()))?;

    Ok(Path::new(SAVE_DIR).join(sav_file).with_extension("sav"))
}

fn get_state_file_path(rom_file: &Path, slot: usize) -> Result<PathBuf> {
    let state_file = rom_file
        .file_stem()
        .ok_or_else(|| anyhow!("Invalid file name: {}", rom_file.display()))?;
    let state_file = format!("{}-{slot}", state_file.to_string_lossy());

    let state_dir = Path::new(STATE_DIR);
    if !state_dir.exists() {
        fs::create_dir_all(state_dir)?;
    } else if !state_dir.is_dir() {
        bail!("`{}` is not a directory", state_dir.display());
    }

    Ok(state_dir.join(state_file).with_extension("state"))
}

pub fn load_rom(file: &Path) -> Result<Rom> {
    let extension = file.extension().and_then(|e| e.to_str());

    match extension {
        Some("zip") => {
            let mut archive = zip::ZipArchive::new(File::open(file)?)?;
            let mut found = vec![];
            for i in 0..archive.len() {
                let file = archive.by_index(i)?;
                let path = match file.enclosed_name() {
                    Some(path) => path.to_owned(),
                    None => continue,
                };

                let extension = path.extension().and_then(|e| e.to_str());

                match extension {
                    Some("gb" | "gbc") => found.push(i),
                    _ => {}
                }
            }

            if found.is_empty() {
                bail!("No GB/GBC file found in archive");
            }

            let mut file = archive.by_index(found[0])?;

            if found.len() > 1 {
                info!(
                    "Multiple GB/GBC files found in archive. Open `{}`.",
                    file.enclosed_name().unwrap().display()
                );
            }

            info!(
                "Loading ROM from: `{}`.",
                file.enclosed_name().unwrap().display()
            );
            let mut bytes = vec![];
            io::copy(&mut file, &mut bytes)?;
            Rom::from_bytes(&bytes)
        }
        Some("gb" | "gbc") => {
            info!("Loading ROM from: `{}`.", file.display());
            let bytes = std::fs::read(file)?;
            Rom::from_bytes(&bytes)
        }
        _ => bail!("Unsupported file extension"),
    }
}

pub fn load_backup_ram(file: &Path) -> Result<Option<Vec<u8>>> {
    let save_file_path = get_save_file_path(file)?;

    Ok(if save_file_path.is_file() {
        info!("Loading backup RAM: `{}`", save_file_path.display());
        Some(std::fs::read(save_file_path)?)
    } else {
        None
    })
}

pub fn save_backup_ram(rom_file: &Path, ram: &[u8]) -> Result<()> {
    let save_file_path = get_save_file_path(rom_file)?;

    if !save_file_path.exists() {
        info!("Creating backup RAM file: `{}`", save_file_path.display());
    } else {
        info!(
            "Overwriting backup RAM file: `{}`",
            save_file_path.display()
        );
    }
    atomic_write_file(&save_file_path, ram)
}

pub fn save_state_data(rom_file: &Path, slot: usize, data: &[u8]) -> Result<()> {
    atomic_write_file(&get_state_file_path(rom_file, slot)?, data)?;
    info!("Saved state to slot {slot}");
    Ok(())
}

pub fn load_state_data(rom_file: &Path, slot: usize) -> Result<Vec<u8>> {
    let ret = fs::read(get_state_file_path(rom_file, slot)?)?;
    info!("Loaded state from slot {slot}");
    Ok(ret)
}

pub fn print_rom_info(info: &[(&str, String)]) {
    use prettytable::{cell, format, row, Table};

    let mut table = Table::new();
    for (k, v) in info {
        table.add_row(row![k, v]);
    }
    table.set_titles(row!["ROM File Info"]);
    table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);

    for line in table.to_string().lines() {
        info!("{line}");
    }
}
