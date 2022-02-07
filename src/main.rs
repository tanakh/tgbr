use anyhow::{anyhow, bail, Result};
use log::info;
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
    config::Config,
    gameboy::GameBoy,
    interface::{Input, Pad},
    rom::Rom,
};

const SCALING: u32 = 4;
const FPS: f64 = 60.0;

const DMG_PALETTE: [tgbr::interface::Color; 4] = {
    use tgbr::interface::Color;
    [
        // Color::new(155, 188, 15),
        // Color::new(139, 172, 15),
        // Color::new(48, 98, 48),
        // Color::new(15, 56, 15),

        // Color::new(155, 188, 15),
        // Color::new(136, 170, 10),
        // Color::new(48, 98, 48),
        // Color::new(15, 56, 15)
        Color::new(160, 207, 10),
        Color::new(140, 191, 10),
        Color::new(46, 115, 32),
        Color::new(0, 63, 0),
    ]
};

#[argopt::cmd]
fn main(
    /// Path to Boot ROM
    #[opt(long)]
    boot_rom: Option<PathBuf>,
    /// Path to Cartridge ROM
    file: PathBuf,
) -> Result<()> {
    env_logger::builder().format_timestamp(None).init();

    let rom = load_rom(&file)?;
    rom.info();

    let boot_rom = if let Some(boot_rom) = boot_rom {
        Some(fs::read(boot_rom)?)
    } else {
        None
    };

    let config = Config::default()
        .set_dmg_palette(&DMG_PALETTE)
        .set_boot_rom(boot_rom);

    let mut gb = GameBoy::new(rom, &config)?;

    let (width, height) = {
        let buf = gb.frame_buffer().borrow();
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
    device.queue(&vec![0; 2048 * 2]);
    device.resume();

    let input_manager = InputManager::new(&sdl_context, KeyConfig::default())?;

    let mut event_pump = sdl_context.event_pump().map_err(|e| anyhow!("{e}"))?;

    let mut timer = Timer::new();
    // let mut audio_filter = AudioFilter::new();

    while process_events(&mut event_pump) {
        let input = input_manager.get_input(&event_pump);

        gb.set_input(&input);
        gb.exec_frame();

        surface.with_lock_mut(|r| {
            let buf = gb.frame_buffer().borrow();

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

        let audio_buf = gb.audio_buffer().borrow();
        assert!(
            (799..=801).contains(&audio_buf.buf.len()),
            "invalid generated audio length: {}",
            audio_buf.buf.len()
        );

        while device.size() > 2048 * 2 * 2 {
            std::thread::sleep(Duration::from_millis(1));
        }

        device.queue(
            &audio_buf
                .buf
                .iter()
                .map(|s| [s.right, s.left])
                .flatten()
                .collect::<Vec<_>>(),
        );

        // FIXME
        timer.wait_for_frame(FPS * 2.0);
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

struct InputManager {
    key_config: KeyConfig,
    controllers: Vec<GameController>,
}

struct KeyConfig {
    up: Vec<Key>,
    down: Vec<Key>,
    left: Vec<Key>,
    right: Vec<Key>,
    a: Vec<Key>,
    b: Vec<Key>,
    start: Vec<Key>,
    select: Vec<Key>,
}

enum Key {
    Keyboard {
        scancode: keyboard::Scancode,
    },
    GameController {
        id: usize,
        button: controller::Button,
    },
}

impl Default for KeyConfig {
    fn default() -> Self {
        use sdl2::keyboard::Scancode;

        Self {
            up: vec![
                Key::Keyboard {
                    scancode: Scancode::Up,
                },
                Key::GameController {
                    id: 0,
                    button: controller::Button::DPadUp,
                },
            ],
            down: vec![
                Key::Keyboard {
                    scancode: Scancode::Down,
                },
                Key::GameController {
                    id: 0,
                    button: controller::Button::DPadDown,
                },
            ],
            left: vec![
                Key::Keyboard {
                    scancode: Scancode::Left,
                },
                Key::GameController {
                    id: 0,
                    button: controller::Button::DPadLeft,
                },
            ],
            right: vec![
                Key::Keyboard {
                    scancode: Scancode::Right,
                },
                Key::GameController {
                    id: 0,
                    button: controller::Button::DPadRight,
                },
            ],
            a: vec![
                Key::Keyboard {
                    scancode: Scancode::Z,
                },
                Key::GameController {
                    id: 0,
                    button: controller::Button::A,
                },
            ],
            b: vec![
                Key::Keyboard {
                    scancode: Scancode::X,
                },
                Key::GameController {
                    id: 0,
                    button: controller::Button::X,
                },
            ],
            start: vec![
                Key::Keyboard {
                    scancode: Scancode::Return,
                },
                Key::GameController {
                    id: 0,
                    button: controller::Button::Start,
                },
            ],
            select: vec![
                Key::Keyboard {
                    scancode: Scancode::Backspace,
                },
                Key::GameController {
                    id: 0,
                    button: controller::Button::Back,
                },
            ],
        }
    }
}

impl InputManager {
    fn new(sdl: &Sdl, key_config: KeyConfig) -> Result<Self> {
        let gcs = sdl.game_controller().map_err(|e| anyhow!("{e}"))?;

        let controllers = (0..(gcs.num_joysticks().map_err(|e| anyhow!("{e}"))?))
            .map(|id| gcs.open(id))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            key_config,
            controllers,
        })
    }

    fn get_input(&self, e: &EventPump) -> Input {
        let kbstate = keyboard::KeyboardState::new(e);

        let pressed = |keys: &Vec<Key>| {
            keys.iter().any(|k| match k {
                Key::Keyboard { scancode } => kbstate.is_scancode_pressed(*scancode),
                Key::GameController { id, button } => self
                    .controllers
                    .get(*id)
                    .map_or(false, |r| r.button(*button)),
            })
        };

        let pad = Pad {
            up: pressed(&self.key_config.up),
            down: pressed(&self.key_config.down),
            left: pressed(&self.key_config.left),
            right: pressed(&self.key_config.right),
            a: pressed(&self.key_config.a),
            b: pressed(&self.key_config.b),
            start: pressed(&self.key_config.start),
            select: pressed(&self.key_config.select),
        };

        Input { pad }
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
