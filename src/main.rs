mod input;
mod timer;

use anyhow::{anyhow, bail, Result};
use log::{info, log_enabled};
use sdl2::{
    audio::{AudioQueue, AudioSpecDesired},
    event::Event,
    pixels::{Color, PixelFormatEnum},
    rect::Rect,
    EventPump,
};
use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    time::Duration,
};

use tgbr_core::{Config, GameBoy, Rom};

use input::{HotKey, HotKeys, InputManager, KeyConfig};

const SAVE_DIR: &str = "./save";
const STATE_DIR: &str = "./state";

const SCALING: u32 = 4;
const FPS: f64 = 60.0;

const DMG_PALETTE: [tgbr_core::Color; 4] = {
    use tgbr_core::Color;
    [
        // Color::new(155, 188, 15),
        // Color::new(139, 172, 15),
        // Color::new(48, 98, 48),
        // Color::new(15, 56, 15),

        // Color::new(155, 188, 15),
        // Color::new(136, 170, 10),
        // Color::new(48, 98, 48),
        // Color::new(15, 56, 15)

        // Color::new(160, 207, 10),
        // Color::new(140, 191, 10),
        // Color::new(46, 115, 32),
        // Color::new(0, 63, 0),
        Color::new(200, 200, 168),
        Color::new(164, 164, 140),
        Color::new(104, 104, 84),
        Color::new(40, 40, 20),
    ]
};

#[argopt::cmd]
fn main(
    /// Path to Boot ROM
    #[opt(long)]
    boot_rom: Option<PathBuf>,
    /// Path to Cartridge ROM
    rom_file: PathBuf,
) -> Result<()> {
    env_logger::builder().format_timestamp(None).init();

    let rom = load_rom(&rom_file)?;
    if log_enabled!(log::Level::Info) {
        print_rom_info(&rom.info());
    }

    let backup_ram = load_backup_ram(&rom_file)?;

    let boot_rom = if let Some(boot_rom) = boot_rom {
        Some(fs::read(boot_rom)?)
    } else {
        None
    };

    let config = Config::default()
        .set_dmg_palette(&DMG_PALETTE)
        .set_boot_rom(boot_rom);

    let mut gb = GameBoy::new(rom, backup_ram, &config)?;

    let (width, height) = {
        let buf = gb.frame_buffer();
        (buf.width, buf.height)
    };

    let screen_width = width as u32 * SCALING;
    let screen_height = height as u32 * SCALING;

    let sdl_context = sdl2::init().map_err(|e| anyhow!("{e}"))?;
    let video_subsystem = sdl_context.video().map_err(|e| anyhow!("{e}"))?;

    let window = video_subsystem
        .window("TGB-R", screen_width, screen_height)
        .build()?;

    let mut canvas = window.into_canvas().present_vsync().build()?;
    let texture_creator = canvas.texture_creator();

    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    canvas.present();

    let ttf_context = sdl2::ttf::init().map_err(|e| anyhow!("{e}"))?;
    let font = ttf_context
        .load_font("./assets/fonts/Inconsolata-Regular.ttf", 32)
        .map_err(|e| anyhow!("{e}"))?;

    let mut surface =
        sdl2::surface::Surface::new(width as u32, height as u32, PixelFormatEnum::RGB24)
            .map_err(|e| anyhow!("{e}"))?;

    let audio_subsystem = sdl_context.audio().map_err(|e| anyhow!("{e}"))?;
    let desired_spec = AudioSpecDesired {
        freq: Some(48000),
        channels: Some(2),
        samples: Some(2048),
    };
    let device: AudioQueue<i16> = audio_subsystem
        .open_queue(None, &desired_spec)
        .map_err(|e| anyhow!("{e}"))?;
    device
        .queue_audio(&vec![0; 2048 * 2])
        .map_err(|e| anyhow!("{e}"))?;
    device.resume();

    let key_config = KeyConfig::default();
    let hotkeys = HotKeys::default();
    let mut input_manager = InputManager::new(&sdl_context, &key_config, &hotkeys)?;

    let mut event_pump = sdl_context.event_pump().map_err(|e| anyhow!("{e}"))?;

    let mut timer = timer::Timer::new();

    let mut frames = 0;
    let mut state_save_slot = 0;

    while process_events(&mut event_pump) {
        input_manager.update(&event_pump);
        let input = input_manager.input();
        let is_turbo = input_manager.hotkey(HotKey::Turbo).pressed();

        if input_manager.hotkey(HotKey::StateSave).pushed() {
            let data = gb.save_state();
            save_state_data(&rom_file, state_save_slot, &data)?;
        }

        if input_manager.hotkey(HotKey::StateLoad).pushed() {
            let data = load_state_data(&rom_file, state_save_slot)?;
            gb.load_state(&data)?;
        }

        gb.set_input(&input);
        gb.exec_frame();

        frames += 1;

        if !is_turbo || frames % 5 == 0 {
            surface.with_lock_mut(|r| {
                let buf = gb.frame_buffer();

                for y in 0..height {
                    for x in 0..width {
                        let ix = y * width + x;
                        let p = buf.get(x, y);
                        r[ix * 3 + 0] = p.r;
                        r[ix * 3 + 1] = p.g;
                        r[ix * 3 + 2] = p.b;
                    }
                }
            });

            let texture = surface.as_texture(&texture_creator)?;
            canvas
                .copy(&texture, None, None)
                .map_err(|e| anyhow!("{e}"))?;

            {
                let fps_tex = font
                    .render(&format!("{:.02}", timer.fps()))
                    .blended(Color::RED)?
                    .as_texture(&texture_creator)?;

                let (w, h) = {
                    let q = fps_tex.query();
                    (q.width, q.height)
                };

                canvas
                    .copy(
                        &fps_tex,
                        None,
                        Rect::new(screen_width as i32 - w as i32, 0, w, h),
                    )
                    .map_err(|e| anyhow!("{e}"))?;
            }

            canvas.present();
        }

        let queue_audio = if is_turbo {
            device.size() < 2048 * 2 * 2
        } else {
            while device.size() > 2048 * 2 * 2 {
                std::thread::sleep(Duration::from_millis(1));
            }
            true
        };

        if queue_audio {
            let audio_buf = gb.audio_buffer();
            assert!(
                (799..=801).contains(&audio_buf.buf.len()),
                "invalid generated audio length: {}",
                audio_buf.buf.len()
            );
            device
                .queue_audio(
                    &audio_buf
                        .buf
                        .iter()
                        .map(|s| [s.right, s.left])
                        .flatten()
                        .collect::<Vec<_>>(),
                )
                .map_err(|e| anyhow!("{e}"))?;
        }

        // FIXME
        timer.wait_for_frame(if !is_turbo { 999.9 } else { 999.0 });
    }

    if let Some(ram) = gb.backup_ram() {
        save_backup_ram(&rom_file, &ram)?;
    } else {
        info!("No backup RAM to save");
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

fn load_backup_ram(file: &Path) -> Result<Option<Vec<u8>>> {
    let save_file_path = get_save_file_path(file)?;

    Ok(if save_file_path.is_file() {
        info!("Loading backup RAM: `{}`", save_file_path.display());
        Some(std::fs::read(save_file_path)?)
    } else {
        None
    })
}

fn save_backup_ram(rom_file: &Path, ram: &[u8]) -> Result<()> {
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

fn atomic_write_file(file: &Path, data: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut f = tempfile::NamedTempFile::new()?;
    f.write_all(data)?;
    f.persist(file)?;
    Ok(())
}

fn save_state_data(rom_file: &Path, slot: usize, data: &[u8]) -> Result<()> {
    atomic_write_file(&get_state_file_path(rom_file, slot)?, data)?;
    info!("Saved state to slot {slot}");
    Ok(())
}

fn load_state_data(rom_file: &Path, slot: usize) -> Result<Vec<u8>> {
    let ret = fs::read(get_state_file_path(rom_file, slot)?)?;
    info!("Loaded state from slot {slot}");
    Ok(ret)
}

fn print_rom_info(info: &[(&str, String)]) {
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

fn process_events(event_pump: &mut EventPump) -> bool {
    for event in event_pump.poll_iter() {
        match event {
            Event::Quit { .. }
            | Event::KeyDown {
                keycode: Some(sdl2::keyboard::Keycode::Escape),
                ..
            } => return false,
            _ => {}
        }
    }
    true
}
