use anyhow::{bail, Result};
use log::info;
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
};

use tgbr::{gameboy::GameBoy, rom::Rom};

#[argopt::cmd]
fn main(file: PathBuf) -> Result<()> {
    env_logger::builder().format_timestamp(None).init();

    let rom = load_rom(&file)?;
    rom.info();

    let mut gb = GameBoy::new(rom, false);

    loop {
        gb.exec_frame();
    }

    Ok(())
}

fn load_rom(file: &Path) -> Result<Rom> {
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
                    _ => bail!("Unsupported file extension"),
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

            let mut bytes = vec![];
            io::copy(&mut file, &mut bytes)?;
            Rom::from_bytes(&bytes)
        }
        Some("gb" | "gbc") => {
            let bytes = std::fs::read(file)?;
            Rom::from_bytes(&bytes)
        }
        _ => bail!("Unsupported file extension"),
    }
}
