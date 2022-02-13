mod input;
mod timer;

use anyhow::{anyhow, bail, Result};
use log::{error, info, log_enabled};
use sdl2::{
    audio::{AudioQueue, AudioSpecDesired},
    event::Event,
    pixels::{Color, PixelFormatEnum},
    rect::Rect,
    render::Texture,
    surface::Surface,
    EventPump,
};
use std::{
    cmp::min,
    collections::VecDeque,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    time::Duration,
};

use tgbr_core::{Config, FrameBuffer, GameBoy, Rom};

use input::{HotKey, HotKeys, InputManager, KeyConfig, PadButton};

const SAVE_DIR: &str = "./save";
const STATE_DIR: &str = "./state";

const SCALING: usize = 4;
const FPS: f64 = 60.0;
const FRAME_SKIP_ON_TURBO: usize = 5;
const AUDIO_FREQUENCY: usize = 48000;
const AUDIO_BUFFER_SAMPLES: usize = 2048;

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

const MAX_AUTO_STATE_SAVES: usize = 10 * 60;
const AUTO_STATE_SAVE_FREQUENCY: usize = 2 * 60;

#[argopt::cmd]
fn main(
    /// Path to Boot ROM
    #[opt(long)]
    boot_rom: Option<PathBuf>,
    /// Path to Cartridge ROM
    rom_file: PathBuf,
) -> Result<()> {
    env_logger::builder().format_timestamp(None).init();
    App::new(&rom_file, &boot_rom)?.run()
}

enum AppState {
    Running,
    Paused,
    Rewinding,
}

struct App {
    gb: GameBoy,
    rom_file: PathBuf,

    state: AppState,
    frames: usize,
    timer: timer::Timer,
    state_save_slot: usize,
    auto_saved_states: VecDeque<AutoSavedState>,
    rewind_pos: usize,

    show_fps: bool,

    screen_width: usize,
    screen_height: usize,
    canvas: sdl2::render::Canvas<sdl2::video::Window>,
    surface: sdl2::surface::Surface<'static>,
    texture_creator: sdl2::render::TextureCreator<sdl2::video::WindowContext>,
    audio_queue: AudioQueue<i16>,
    event_pump: EventPump,
    input_manager: InputManager,
}

struct AutoSavedState {
    thumbnail: Texture,
    data: Vec<u8>,
}

impl App {
    fn new(rom_file: &Path, boot_rom: &Option<PathBuf>) -> Result<Self> {
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

        let gb = GameBoy::new(rom, backup_ram, &config)?;

        let (width, height) = {
            let buf = gb.frame_buffer();
            (buf.width, buf.height)
        };

        let screen_width = width * SCALING;
        let screen_height = height * SCALING;

        let sdl_context = sdl2::init().map_err(|e| anyhow!("{e}"))?;
        let video_subsystem = sdl_context.video().map_err(|e| anyhow!("{e}"))?;

        let window = video_subsystem
            .window("TGB-R", screen_width as u32, screen_height as u32)
            .build()?;

        let canvas = window.into_canvas().present_vsync().build()?;
        let texture_creator = canvas.texture_creator();

        let surface =
            sdl2::surface::Surface::new(width as u32, height as u32, PixelFormatEnum::RGB24)
                .map_err(|e| anyhow!("{e}"))?;

        let audio_subsystem = sdl_context.audio().map_err(|e| anyhow!("{e}"))?;
        let desired_spec = AudioSpecDesired {
            freq: Some(AUDIO_FREQUENCY as _),
            channels: Some(2),
            samples: Some(AUDIO_BUFFER_SAMPLES as _),
        };
        let audio_queue: AudioQueue<i16> = audio_subsystem
            .open_queue(None, &desired_spec)
            .map_err(|e| anyhow!("{e}"))?;
        audio_queue
            .queue_audio(&vec![0; AUDIO_BUFFER_SAMPLES * 2])
            .map_err(|e| anyhow!("{e}"))?;
        audio_queue.resume();

        let key_config = KeyConfig::default();
        let hotkeys = HotKeys::default();
        let input_manager = InputManager::new(&sdl_context, &key_config, &hotkeys)?;
        let event_pump = sdl_context.event_pump().map_err(|e| anyhow!("{e}"))?;

        Ok(Self {
            gb,
            rom_file: rom_file.to_owned(),

            state: AppState::Running,
            frames: 0,
            timer: timer::Timer::new(),
            state_save_slot: 0,
            auto_saved_states: VecDeque::new(),
            rewind_pos: 0,

            show_fps: true,

            screen_width,
            screen_height,
            canvas,
            surface,
            texture_creator,
            audio_queue,
            event_pump,
            input_manager,
        })
    }

    fn run(&mut self) -> Result<()> {
        let ttf_context = sdl2::ttf::init().map_err(|e| anyhow!("{e}"))?;
        let font = ttf_context
            .load_font("./assets/fonts/PixelMplus12-Regular.ttf", 36)
            .map_err(|e| anyhow!("{e}"))?;

        while process_events(&mut self.event_pump) {
            self.input_manager.update(&self.event_pump);
            self.dispatch_event()?;

            match self.state {
                AppState::Running => {
                    self.running(&font)?;
                    self.frames += 1;
                }
                AppState::Rewinding => {
                    self.rewinding()?;
                }
                AppState::Paused => todo!(),
            }
        }

        if let Some(ram) = self.gb.backup_ram() {
            save_backup_ram(&self.rom_file, &ram)?;
        } else {
            info!("No backup RAM to save");
        }

        Ok(())
    }

    fn dispatch_event(&mut self) -> Result<()> {
        match self.state {
            AppState::Running => {
                if self.input_manager.hotkey(HotKey::Reset).pushed() {
                    self.gb.reset();
                    info!("Reset machine");
                }

                if self.input_manager.hotkey(HotKey::StateSave).pushed() {
                    let data = self.gb.save_state();
                    save_state_data(&self.rom_file, self.state_save_slot, &data)?;
                }

                if self.input_manager.hotkey(HotKey::StateLoad).pushed() {
                    let data = load_state_data(&self.rom_file, self.state_save_slot)?;
                    let res = self.gb.load_state(&data);
                    if let Err(e) = res {
                        error!("Failed to load state: {}", e);
                    }
                }

                if self.input_manager.hotkey(HotKey::NextSlot).pushed() {
                    self.state_save_slot += 1;
                    info!("State save slot changed: {}", self.state_save_slot);
                }

                if self.input_manager.hotkey(HotKey::PrevSlot).pushed() {
                    self.state_save_slot = self.state_save_slot.saturating_sub(1);
                    info!("State save slot changed: {}", self.state_save_slot);
                }

                if self.input_manager.hotkey(HotKey::Rewind).pushed() {
                    self.auto_state_save()?;
                    self.state = AppState::Rewinding;
                    self.rewind_pos = self.auto_saved_states.len() - 1;
                }
            }
            AppState::Rewinding => {
                if self.input_manager.pad_button(PadButton::Left).pushed() {
                    self.rewind_pos = self.rewind_pos.saturating_sub(1);
                }
                if self.input_manager.pad_button(PadButton::Right).pushed() {
                    self.rewind_pos = min(self.auto_saved_states.len() - 1, self.rewind_pos + 1);
                }
                if self.input_manager.pad_button(PadButton::A).pushed()
                    || self.input_manager.pad_button(PadButton::Start).pushed()
                {
                    self.gb
                        .load_state(&self.auto_saved_states[self.rewind_pos].data)?;
                    while self.auto_saved_states.len() > self.rewind_pos {
                        let st = self.auto_saved_states.pop_back().unwrap();
                        unsafe { st.thumbnail.destroy() };
                    }
                    self.state = AppState::Running;
                    self.frames = 0;
                    info!("State rewinded");
                }
                if self.input_manager.pad_button(PadButton::B).pushed() {
                    self.state = AppState::Running;
                }
            }
            AppState::Paused => todo!(),
        }

        Ok(())
    }

    fn running(&mut self, font: &sdl2::ttf::Font<'_, '_>) -> Result<()> {
        let input = self.input_manager.input();
        self.gb.set_input(&input);
        self.gb.exec_frame();

        if self.frames % AUTO_STATE_SAVE_FREQUENCY == 0 {
            self.auto_state_save()?;
        }

        let is_turbo = self.input_manager.hotkey(HotKey::Turbo).pressed();

        if !is_turbo || self.frames % FRAME_SKIP_ON_TURBO == 0 {
            let texture = self.to_texture(self.gb.frame_buffer())?;
            self.canvas
                .copy(&texture, None, None)
                .map_err(|e| anyhow!("{e}"))?;
            unsafe { texture.destroy() };

            if self.show_fps {
                self.render_fps(font)?;
            }
            self.canvas.present();
        }

        if !is_turbo {
            self.sync_audio();
        }
        self.queue_audio()?;

        let fps = if !is_turbo { 999.9 } else { 999.0 };
        self.timer.wait_for_frame(fps);
        Ok(())
    }

    fn rewinding(&mut self) -> Result<()> {
        self.canvas.set_draw_color(Color::RGB(0, 0, 0));
        self.canvas.clear();

        self.canvas.set_draw_color(Color::RGB(64, 64, 64));
        self.canvas
            .fill_rect(self.convert_coord((0.5, 5.0 / 6.0), 1.0, 1.0 / 3.0))
            .map_err(|e| anyhow!("{e}"))?;

        self.canvas
            .copy(
                &self.auto_saved_states[self.rewind_pos].thumbnail,
                None,
                self.convert_coord((0.5, 1.0 / 3.0), 2.0 / 3.0 * 0.95, 2.0 / 3.0 * 0.95),
            )
            .map_err(|e| anyhow!("{e}"))?;

        self.canvas.set_draw_color(Color::RGB(200, 200, 200));
        self.canvas
            .fill_rect(self.convert_coord((0.5, 5.0 / 6.0), 0.2, 0.2))
            .map_err(|e| anyhow!("{e}"))?;

        for i in -2..=2 {
            let ix = self.rewind_pos as isize + i;
            if !(ix >= 0 && ix < self.auto_saved_states.len() as isize) {
                continue;
            }
            let ix = ix as usize;
            let x = 0.5 + (i * 2) as f64 * 0.1;
            let y = 5.0 / 6.0;
            let scale = 0.2 * if i == 0 { 0.95 } else { 0.85 };

            self.canvas
                .copy(
                    &self.auto_saved_states[ix].thumbnail,
                    None,
                    self.convert_coord((x, y), scale, scale),
                )
                .map_err(|e| anyhow!("{e}"))?;
        }

        self.canvas.present();
        self.timer.wait_for_frame(FPS);

        Ok(())
    }

    fn convert_coord(&self, pt: (f64, f64), w: f64, h: f64) -> Option<Rect> {
        let cx = pt.0;
        let cy = pt.1;
        let l = cx - 0.5 * w;
        let u = cy - 0.5 * h;
        Some(
            (
                (l * self.screen_width as f64).round() as i32,
                (u * self.screen_height as f64).round() as i32,
                (w * self.screen_width as f64).round() as u32,
                (h * self.screen_height as f64).round() as u32,
            )
                .into(),
        )
    }

    fn auto_state_save(&mut self) -> Result<()> {
        let state = AutoSavedState {
            data: self.gb.save_state(),
            thumbnail: self.to_texture(self.gb.frame_buffer())?,
        };
        self.auto_saved_states.push_back(state);
        if self.auto_saved_states.len() > MAX_AUTO_STATE_SAVES {
            let st = self.auto_saved_states.pop_front().unwrap();
            unsafe { st.thumbnail.destroy() };
        }

        Ok(())
    }

    fn to_texture(&self, frame_buffer: &FrameBuffer) -> Result<Texture> {
        let mut surface: Surface<'static> = sdl2::surface::Surface::new(
            frame_buffer.width as u32,
            frame_buffer.height as u32,
            PixelFormatEnum::RGB24,
        )
        .map_err(|e| anyhow!("{e}"))?;

        self.copy_to_surface(&mut surface, frame_buffer);
        let ret = surface
            .as_texture(&self.texture_creator)
            .map_err(|e| anyhow!("{e}"))?;
        Ok(ret)
    }

    fn copy_to_surface(&self, surface: &mut Surface, frame_buffer: &FrameBuffer) {
        surface.with_lock_mut(|r| {
            for y in 0..frame_buffer.height {
                for x in 0..frame_buffer.width {
                    let ix = y * frame_buffer.width + x;
                    let p = frame_buffer.get(x, y);
                    r[ix * 3 + 0] = p.r;
                    r[ix * 3 + 1] = p.g;
                    r[ix * 3 + 2] = p.b;
                }
            }
        });
    }

    fn sync_audio(&mut self) {
        while self.audio_queue.size() as usize >= AUDIO_BUFFER_SAMPLES * 2 * 2 {
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn queue_audio(&mut self) -> Result<()> {
        if (self.audio_queue.size() as usize) >= AUDIO_BUFFER_SAMPLES * 2 * 2 {
            return Ok(());
        }

        let audio_buf = self.gb.audio_buffer();
        assert!(
            (799..=801).contains(&audio_buf.buf.len()),
            "invalid generated audio length: {}",
            audio_buf.buf.len()
        );
        self.audio_queue
            .queue_audio(
                &audio_buf
                    .buf
                    .iter()
                    .map(|s| [s.right, s.left])
                    .flatten()
                    .collect::<Vec<_>>(),
            )
            .map_err(|e| anyhow!("{e}"))?;
        Ok(())
    }

    fn render_fps(&mut self, font: &sdl2::ttf::Font<'_, '_>) -> Result<()> {
        let text = format!("{:5.02}", self.timer.fps());
        let fps_tex = font
            .render(&text[0..5])
            .blended(Color::WHITE)?
            .as_texture(&self.texture_creator)?;

        let (w, h) = {
            let q = fps_tex.query();
            (q.width, q.height)
        };

        let r1 = Rect::new(
            self.screen_width as i32 - w as i32 * 11 / 10,
            0,
            w * 11 / 10,
            h,
        );
        let r2 = Rect::new(self.screen_width as i32 - w as i32, 0, w, h);

        self.canvas.set_draw_color(Color::RGBA(0, 0, 0, 192));
        self.canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
        self.canvas.fill_rect(r1).map_err(|e| anyhow!("{e}"))?;

        self.canvas
            .copy(&fps_tex, None, r2)
            .map_err(|e| anyhow!("{e}"))?;

        unsafe { fps_tex.destroy() };

        Ok(())
    }
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
