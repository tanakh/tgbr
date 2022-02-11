use anyhow::{anyhow, bail, Result};
use log::{info, log_enabled};
use sdl2::{
    audio::{AudioQueue, AudioSpecDesired},
    controller::{self, GameController},
    event::Event,
    keyboard::{self, Keycode},
    pixels::{Color, PixelFormatEnum},
    rect::Rect,
    EventPump, Sdl,
};
use std::{
    collections::VecDeque,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    time::Duration,
};

use tgbr::{
    interface::{Input, Pad},
    Config, GameBoy, Rom,
};

const SCALING: u32 = 4;
const FPS: f64 = 60.0;

const DMG_PALETTE: [tgbr::Color; 4] = {
    use tgbr::Color;
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

    let mut timer = Timer::new();
    // let mut audio_filter = AudioFilter::new();

    let mut frames = 0;

    while process_events(&mut event_pump) {
        input_manager.update(&event_pump);
        let input = input_manager.input();
        let is_turbo = input_manager.hotkey(HotKey::Turbo).pressed();

        const SS_FILE_NAME: &str = "save.state";

        if input_manager.hotkey(HotKey::StateSave).pushed() {
            let data = gb.save_state();
            std::fs::write(SS_FILE_NAME, data)?;
        }

        if input_manager.hotkey(HotKey::StateLoad).pushed() {
            let data = std::fs::read(SS_FILE_NAME)?;
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

const SAVE_DIR: &str = "./save";

fn load_backup_ram(file: &Path) -> Result<Option<Vec<u8>>> {
    let sav_file = file
        .file_stem()
        .ok_or_else(|| anyhow!("Invalid file name: {}", file.display()))?;

    let save_file_path = Path::new(SAVE_DIR).join(sav_file).with_extension("sav");

    Ok(if save_file_path.is_file() {
        info!("Loading backup RAM: `{}`", save_file_path.display());
        Some(std::fs::read(save_file_path)?)
    } else {
        None
    })
}

fn save_backup_ram(file: &Path, ram: &[u8]) -> Result<()> {
    let sav_file = file
        .file_stem()
        .ok_or_else(|| anyhow!("Invalid file name: {}", file.display()))?;

    let save_dir = Path::new(SAVE_DIR);
    if !save_dir.exists() {
        fs::create_dir_all(save_dir)?;
    } else if !save_dir.is_dir() {
        bail!("`{}` is not a directory", save_dir.display());
    }

    let save_file_path = save_dir.join(sav_file).with_extension("sav");

    if !save_file_path.exists() {
        info!("Creating backup RAM file: `{}`", save_file_path.display());
    } else {
        info!(
            "Overwriting backup RAM file: `{}`",
            save_file_path.display()
        );
    }
    // Atomic write to save file
    use std::io::Write;
    let mut f = tempfile::NamedTempFile::new()?;
    f.write_all(ram)?;
    f.persist(save_file_path)?;

    Ok(())
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
                keycode: Some(Keycode::Escape),
                ..
            } => return false,
            _ => {}
        }
    }
    true
}

#[derive(Clone)]
enum KeyAssign {
    Keyboard {
        scancode: keyboard::Scancode,
    },
    PadButton {
        id: usize,
        button: controller::Button,
    },
    PadAxis {
        id: usize,
        axis: controller::Axis,
    },
    All(Vec<KeyAssign>),
    Any(Vec<KeyAssign>),
}

macro_rules! kbd {
    ($scancode:ident) => {
        KeyAssign::Keyboard {
            scancode: sdl2::keyboard::Scancode::$scancode,
        }
    };
}

macro_rules! pad_button {
    ($id:expr, $button:ident) => {
        KeyAssign::PadButton {
            id: $id,
            button: controller::Button::$button,
        }
    };
}

macro_rules! pad_axis {
    ($id:expr, $axis:ident) => {
        KeyAssign::PadAxis {
            id: $id,
            axis: controller::Axis::$axis,
        }
    };
}

macro_rules! any {
    ($($key:expr),* $(,)?) => {
        KeyAssign::Any(vec![$($key),*])
    };
}

macro_rules! all {
    ($($key:expr),* $(,)?) => {
        KeyAssign::All(vec![$($key),*])
    };
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum PadButton {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    Start,
    Select,
}

struct KeyConfig(Vec<(PadButton, KeyAssign)>);

impl Default for KeyConfig {
    fn default() -> Self {
        use PadButton::*;
        Self(vec![
            (Up, any![kbd!(Up), pad_button!(0, DPadUp)]),
            (Down, any![kbd!(Down), pad_button!(0, DPadDown)]),
            (Left, any![kbd!(Left), pad_button!(0, DPadLeft)]),
            (Right, any![kbd!(Right), pad_button!(0, DPadRight)]),
            (A, any![kbd!(Z), pad_button!(0, A)]),
            (B, any![kbd!(X), pad_button!(0, X)]),
            (Start, any![kbd!(Return), pad_button!(0, Start)]),
            (Select, any![kbd!(RShift), pad_button!(0, Back)]),
        ])
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum HotKey {
    Turbo,
    StateSave,
    StateLoad,
    FullScreen,
}

struct HotKeys(Vec<(HotKey, KeyAssign)>);

impl Default for HotKeys {
    fn default() -> Self {
        use HotKey::*;
        Self(vec![
            (Turbo, any![kbd!(Tab), pad_axis!(0, TriggerLeft)]),
            (StateSave, all![kbd!(LCtrl), kbd!(S)]),
            (StateLoad, all![kbd!(LCtrl), kbd!(L)]),
            (FullScreen, all![kbd!(RAlt), kbd!(Return)]),
        ])
    }
}

#[derive(PartialEq, Eq, Clone)]
enum Key {
    PadButton(PadButton),
    HotKey(HotKey),
}

struct KeyState {
    key: Key,
    key_assign: KeyAssign,
    pressed: bool,
    prev_pressed: bool,
}

impl KeyState {
    fn pressed(&self) -> bool {
        self.pressed
    }

    fn pushed(&self) -> bool {
        self.pressed && !self.prev_pressed
    }

    fn update(&mut self, pressed: bool) {
        self.prev_pressed = self.pressed;
        self.pressed = pressed;
    }
}

struct InputManager {
    controllers: Vec<GameController>,
    key_states: Vec<KeyState>,
}

static NULL_KEY: KeyState = KeyState {
    key: Key::PadButton(PadButton::Up),
    key_assign: any![],
    pressed: false,
    prev_pressed: false,
};

impl InputManager {
    fn new(sdl: &Sdl, key_config: &KeyConfig, hotkeys: &HotKeys) -> Result<Self> {
        let gcs = sdl.game_controller().map_err(|e| anyhow!("{e}"))?;

        let controllers = (0..(gcs.num_joysticks().map_err(|e| anyhow!("{e}"))?))
            .map(|id| gcs.open(id))
            .collect::<Result<Vec<_>, _>>()?;

        let mut key_states = vec![];

        for r in &key_config.0 {
            key_states.push(KeyState {
                key: Key::PadButton(r.0.clone()),
                key_assign: r.1.clone(),
                pressed: false,
                prev_pressed: false,
            });
        }

        for r in &hotkeys.0 {
            key_states.push(KeyState {
                key: Key::HotKey(r.0.clone()),
                key_assign: r.1.clone(),
                pressed: false,
                prev_pressed: false,
            });
        }

        Ok(Self {
            controllers,
            key_states,
        })
    }

    fn update(&mut self, e: &EventPump) {
        let kbstate = keyboard::KeyboardState::new(e);

        // for i in 0..self.key_states.len() {}
        for r in &mut self.key_states {
            let pressed = check_pressed(&kbstate, &self.controllers, &r.key_assign);
            r.update(pressed);
        }
    }

    fn input(&self) -> Input {
        use PadButton::*;
        Input {
            pad: Pad {
                up: self.pad_button(Up).pressed(),
                down: self.pad_button(Down).pressed(),
                left: self.pad_button(Left).pressed(),
                right: self.pad_button(Right).pressed(),
                a: self.pad_button(A).pressed(),
                b: self.pad_button(B).pressed(),
                start: self.pad_button(Start).pressed(),
                select: self.pad_button(Select).pressed(),
            },
        }
    }

    fn pad_button(&self, pad_button: PadButton) -> &KeyState {
        self.key_states
            .iter()
            .find(|r| &r.key == &Key::PadButton(pad_button))
            .unwrap_or(&NULL_KEY)
    }

    fn hotkey(&self, hotkey: HotKey) -> &KeyState {
        self.key_states
            .iter()
            .find(|r| &r.key == &Key::HotKey(hotkey))
            .unwrap_or(&NULL_KEY)
    }
}

fn check_pressed(
    kbstate: &keyboard::KeyboardState<'_>,
    controllers: &[GameController],
    key: &KeyAssign,
) -> bool {
    use KeyAssign::*;
    match key {
        Keyboard { scancode } => kbstate.is_scancode_pressed(*scancode),
        PadButton { id, button } => controllers.get(*id).map_or(false, |r| r.button(*button)),
        PadAxis { id, axis } => controllers
            .get(*id)
            .map_or(false, |r| dbg!(r.axis(*axis)) > 32767 / 2),
        All(keys) => keys.iter().all(|k| check_pressed(kbstate, controllers, k)),
        Any(keys) => keys.iter().any(|k| check_pressed(kbstate, controllers, k)),
    }
}

use std::time::SystemTime;

struct Timer {
    hist: VecDeque<SystemTime>,
    prev: SystemTime,
}

impl Timer {
    fn new() -> Self {
        Self {
            hist: VecDeque::new(),
            prev: SystemTime::now(),
        }
    }

    fn wait_for_frame(&mut self, fps: f64) {
        let span = 1.0 / fps;

        let elapsed = self.prev.elapsed().unwrap().as_secs_f64();

        if elapsed < span {
            let wait = span - elapsed;
            std::thread::sleep(Duration::from_secs_f64(wait));
        }

        self.prev = SystemTime::now();

        self.hist.push_back(self.prev);
        while self.hist.len() > 60 {
            self.hist.pop_front();
        }
    }

    fn fps(&self) -> f64 {
        if self.hist.len() < 60 {
            return 0.0;
        }

        let span = self.hist.len() - 1;
        let dur = self
            .hist
            .back()
            .unwrap()
            .duration_since(*self.hist.front().unwrap())
            .unwrap()
            .as_secs_f64();

        span as f64 / dur
    }
}
