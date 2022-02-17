mod file;
// mod input;
// mod timer;

use anyhow::Result;
use bevy_kira_audio::{AudioPlugin, AudioStream, AudioStreamPlugin, Frame, StreamedAudio};
use log::{error, info, log_enabled};
// use sdl2::{
//     audio::{AudioQueue, AudioSpecDesired},
//     event::Event,
//     pixels::{Color, PixelFormatEnum},
//     rect::Rect,
//     render::Texture,
//     surface::Surface,
//     EventPump,
// };
use std::{
    collections::VecDeque,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use tgbr_core::{AudioBuffer, Config, GameBoy, Input as GameBoyInput, Pad};

use file::{load_backup_ram, load_rom, print_rom_info, save_backup_ram};

const SCALING: usize = 4;
const FPS: f64 = 60.0;
const FRAME_SKIP_ON_TURBO: usize = 5;
const AUDIO_FREQUENCY: usize = 48000;
const AUDIO_BUFFER_SAMPLES: usize = 2048;

const MAX_AUTO_STATE_SAVES: usize = 10 * 60;
const AUTO_STATE_SAVE_FREQUENCY: usize = 2 * 60;

const DMG_PALETTE: [tgbr_core::Color; 4] = {
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

use bevy::{
    app::AppExit,
    input::prelude::*,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    window::WindowMode,
};
use bevy_egui::{egui, EguiContext, EguiPlugin};

const TIMESTEP_PER_SECOND: f64 = 1.0 / 60.0;

#[argopt::cmd]
fn main(
    /// Path to Boot ROM
    #[opt(long)]
    boot_rom: Option<PathBuf>,
    /// Path to Cartridge ROM
    rom_file: Option<PathBuf>,
) -> Result<()> {
    let mut app = App::new();
    app.insert_resource(WindowDescriptor {
        title: "TGB-R".to_string(),
        width: 160.0 * 4.0,
        height: 24.0 + 144.0 * 4.0,
        ..Default::default()
    })
    .insert_resource(ClearColor(Color::rgb(0.0, 0.0, 0.0)))
    .init_resource::<MenuState>()
    .insert_resource(Msaa { samples: 4 })
    .add_plugins(DefaultPlugins)
    .add_plugin(AudioPlugin)
    .add_plugin(EguiPlugin)
    .add_startup_system(setup)
    .add_system(ui_menu);

    add_gameboy(&mut app);

    if let Some(rom_file) = rom_file {
        let gb = GameBoyState::new(rom_file, boot_rom)?;
        app.insert_resource(gb);
        app.add_state(AppState::Running);
    } else {
        app.add_state(AppState::Unloaded);
    }

    app.run();
    Ok(())
}

fn setup(mut commands: Commands) {
    commands.spawn_bundle(OrthographicCameraBundle::new_2d());
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum AppState {
    Unloaded,
    Running,
    Rewinding,
}

#[derive(Component)]
struct GameBoyState {
    gb: GameBoy,
    rom_file: PathBuf,
}

impl GameBoyState {
    fn new(rom_file: PathBuf, boot_rom: Option<PathBuf>) -> Result<Self> {
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

        Ok(Self { gb, rom_file })
    }
}

impl Drop for GameBoyState {
    fn drop(&mut self) {
        if let Some(ram) = self.gb.backup_ram() {
            if let Err(err) = save_backup_ram(&self.rom_file, &ram) {
                error!("Failed to save backup ram: {err}");
            }
        } else {
            info!("No backup RAM to save");
        }
    }
}

fn add_gameboy(app: &mut App) {
    app.add_plugin(AudioStreamPlugin::<AudioStreamQueue>::default());

    app.add_system(gameboy_input_system.label("input"));
    app.add_system_set(SystemSet::on_enter(AppState::Running).with_system(setup_gameboy_system));
    app.add_system_set(
        SystemSet::on_update(AppState::Running)
            .with_system(gameboy_system)
            .after("input"),
    );
    app.init_resource::<KeyConfig>();
    app.init_resource::<GameBoyInput>();
}

enum KeyAssign {
    KeyCode(KeyCode),
    GamepadButton(GamepadButton),
    GamepadAxis(GamepadAxis, GamepadAxisDir),
    All(Vec<KeyAssign>),
    Any(Vec<KeyAssign>),
}

enum GamepadAxisDir {
    Pos,
    Neg,
}

impl KeyAssign {
    fn pressed(&self, input_state: &InputState<'_>) -> bool {
        match self {
            KeyAssign::KeyCode(keycode) => input_state.input_keycode.pressed(*keycode),
            KeyAssign::GamepadButton(button) => input_state.input_gamepad_button.pressed(*button),
            KeyAssign::GamepadAxis(axis, dir) => {
                input_state
                    .input_gamepad_axis
                    .get(*axis)
                    .map_or(false, |r| match dir {
                        GamepadAxisDir::Pos => r >= 0.5,
                        GamepadAxisDir::Neg => r <= -0.5,
                    })
            }
            KeyAssign::All(ks) => ks.iter().all(|k| k.pressed(input_state)),
            KeyAssign::Any(ks) => ks.iter().any(|k| k.pressed(input_state)),
        }
    }
}

struct KeyConfig {
    up: KeyAssign,
    down: KeyAssign,
    left: KeyAssign,
    right: KeyAssign,
    a: KeyAssign,
    b: KeyAssign,
    start: KeyAssign,
    select: KeyAssign,
}

macro_rules! keycode {
    ($code:ident) => {
        KeyAssign::KeyCode(KeyCode::$code)
    };
}

macro_rules! pad_button {
    ($id:literal, $button:ident) => {
        KeyAssign::GamepadButton(GamepadButton(Gamepad($id), GamepadButtonType::$button))
    };
}

macro_rules! any {
    ($($assign:expr),* $(,)?) => {
        KeyAssign::Any(vec![$($assign),*])
    };
}

impl Default for KeyConfig {
    fn default() -> Self {
        Self {
            up: any!(keycode!(Up), pad_button!(0, DPadUp)),
            down: any!(keycode!(Down), pad_button!(0, DPadDown)),
            left: any!(keycode!(Left), pad_button!(0, DPadLeft)),
            right: any!(keycode!(Right), pad_button!(0, DPadRight)),
            a: any!(keycode!(X), pad_button!(0, South)),
            b: any!(keycode!(Z), pad_button!(0, West)),
            start: any!(keycode!(Return), pad_button!(0, Start)),
            select: any!(keycode!(RShift), pad_button!(0, Select)),
        }
    }
}

impl KeyConfig {
    fn input(&self, input_state: &InputState) -> GameBoyInput {
        GameBoyInput {
            pad: Pad {
                up: self.up.pressed(input_state),
                down: self.down.pressed(input_state),
                left: self.left.pressed(input_state),
                right: self.right.pressed(input_state),
                a: self.a.pressed(input_state),
                b: self.b.pressed(input_state),
                start: self.start.pressed(input_state),
                select: self.select.pressed(input_state),
            },
        }
    }
}

struct InputState<'a> {
    input_keycode: &'a Input<KeyCode>,
    input_gamepad_button: &'a Input<GamepadButton>,
    input_gamepad_axis: &'a Axis<GamepadAxis>,
}

struct GameScreen(Handle<Image>);

#[derive(Debug, Default)]
struct AudioStreamQueue {
    queue: Arc<Mutex<VecDeque<Frame>>>,
}

impl AudioStream for AudioStreamQueue {
    fn next(&mut self, _: f64) -> Frame {
        let mut buffer = self.queue.lock().unwrap();
        buffer.pop_front().unwrap_or_else(|| Frame {
            left: 0.0,
            right: 0.0,
        })
    }
}

fn setup_gameboy_system(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    audio: Res<StreamedAudio<AudioStreamQueue>>,
) {
    info!("Setting up gameboy system");

    let width = 160;
    let height = 144;
    let img = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![0x80; (width * height * 4) as usize],
        TextureFormat::Rgba8UnormSrgb,
    );

    let texture = images.add(img);
    commands.spawn_bundle(SpriteBundle {
        texture: texture.clone(),
        transform: Transform::from_scale(Vec3::new(4.0, 4.0, 1.0))
            .with_translation(Vec3::new(0.0, -12.0, 0.0)),
        ..Default::default()
    });

    commands.insert_resource(GameScreen(texture));

    let audio_queue = Arc::new(Mutex::new(VecDeque::new()));

    audio.stream(AudioStreamQueue {
        queue: Arc::clone(&audio_queue),
    });

    commands.insert_resource(AudioStreamQueue { queue: audio_queue });
}

fn gameboy_input_system(
    key_config: Res<KeyConfig>,
    input_keycode: Res<Input<KeyCode>>,
    input_gamepad_button: Res<Input<GamepadButton>>,
    input_gamepad_axis: Res<Axis<GamepadAxis>>,
    mut input: ResMut<GameBoyInput>,
) {
    *input = key_config.input(&InputState {
        input_keycode: &input_keycode,
        input_gamepad_button: &input_gamepad_button,
        input_gamepad_axis: &input_gamepad_axis,
    });
}

fn gameboy_system(
    // mut commands: Commands,
    screen: ResMut<GameScreen>,
    mut state: ResMut<GameBoyState>,
    mut images: ResMut<Assets<Image>>,
    input: Res<GameBoyInput>,
    audio_queue: Res<AudioStreamQueue>,
) {
    state.gb.set_input(&*input);

    let samples_per_frame = 48000 / 60;

    let mut queue = audio_queue.queue.lock().unwrap();

    let push_audio_queue = |queue: &mut VecDeque<Frame>, audio_buffer: &AudioBuffer| {
        for sample in &audio_buffer.buf {
            queue.push_back(Frame {
                left: sample.left as f32 / 0x7fff as f32,
                right: sample.right as f32 / 0x7fff as f32,
            });
        }
    };

    if queue.len() > samples_per_frame * 4 {
        // execution too fast. wait 1 frame.
        return;
    }

    if queue.len() < samples_per_frame * 2 {
        // execution too slow. run 2 frame for supply enough audio samples.
        state.gb.exec_frame();
        push_audio_queue(&mut *queue, state.gb.audio_buffer());
    }

    state.gb.exec_frame();
    push_audio_queue(&mut *queue, state.gb.audio_buffer());

    let fb = state.gb.frame_buffer();
    let image = images.get_mut(&screen.0).unwrap();

    let width = fb.width;
    let height = fb.height;

    for y in 0..height {
        for x in 0..width {
            let ix = y * width + x;
            let pixel = &mut image.data[ix * 4..ix * 4 + 4];
            pixel[0] = fb.buf[ix].r;
            pixel[1] = fb.buf[ix].g;
            pixel[2] = fb.buf[ix].b;
            pixel[3] = 0xff;
        }
    }
}

#[derive(Default)]
struct MenuState {}

fn ui_menu(
    mut commands: Commands,
    mut egui_ctx: ResMut<EguiContext>,
    // mut ui_state: ResMut<MenuState>,
    mut app_state: ResMut<State<AppState>>,
    mut windows: ResMut<Windows>,
    mut exit: EventWriter<AppExit>,
) {
    let flip_fullscreen = |windows: &mut ResMut<Windows>| {
        let window = windows.get_primary_mut().unwrap();
        let cur_mode = window.mode();
        match cur_mode {
            WindowMode::Windowed => window.set_mode(WindowMode::BorderlessFullscreen),
            WindowMode::BorderlessFullscreen => window.set_mode(WindowMode::Windowed),
            _ => unreachable!(),
        }
    };

    let set_window_scale = |windows: &mut ResMut<Windows>, scale: usize| {
        let window = windows.get_primary_mut().unwrap();

        let w = 160 * scale;
        let h = 144 * scale;

        window.set_resolution(w as f32, h as f32);
    };

    egui::TopBottomPanel::top("top_panel").show(egui_ctx.ctx_mut(), |ui| {
        egui::menu::bar(ui, |ui| {
            egui::menu::menu_button(ui, "File", |ui| {
                if ui.button("Open").clicked() {
                    ui.close_menu();
                    let file = rfd::FileDialog::new()
                        .add_filter("GameBoy ROM file", &["gb", "gbc", "zip"])
                        .pick_file();
                    if let Some(file) = file {
                        match GameBoyState::new(file, None) {
                            Ok(gb) => {
                                commands.insert_resource(gb);

                                if app_state.current() != &AppState::Running {
                                    app_state.set(AppState::Running).unwrap();
                                }
                            }
                            Err(err) => {
                                error!("{err}");
                            }
                        }
                    }
                }
                ui.menu_button("Open Recent", |ui| {
                    let recent_files = &["XXX", "YYY", "ZZZ"];
                    for &recent_file in recent_files {
                        if ui.button(recent_file).clicked() {
                            todo!()
                        }
                    }
                });
                ui.separator();
                if ui.button("Quit").clicked() {
                    exit.send(AppExit);
                }
            });
            egui::menu::menu_button(ui, "Option", |ui| {
                if ui.button("Fullscreen").clicked() {
                    flip_fullscreen(&mut windows);
                    ui.close_menu();
                }
                ui.menu_button("Scale", |ui| {
                    for i in 1..=4 {
                        if ui.button(format!("{}x", i)).clicked() {
                            set_window_scale(&mut windows, i);
                            ui.close_menu();
                        }
                    }
                });
            });
        });
    });

    // egui::CentralPanel::default().show(egui_ctx.ctx_mut(), |ui| {
    //     if ui
    //         .interact(
    //             egui::Rect::EVERYTHING,
    //             egui::Id::null(),
    //             egui::Sense::click(),
    //         )
    //         .double_clicked()
    //     {
    //         flip_fullscreen(&mut windows);
    //     }
    // });
}

// enum AppState {
//     Running,
//     Paused,
//     Rewinding,
// }

// struct App {
//     gb: GameBoy,
//     rom_file: PathBuf,

//     state: AppState,
//     frames: usize,
//     timer: timer::Timer,
//     state_save_slot: usize,
//     auto_saved_states: VecDeque<AutoSavedState>,
//     rewind_pos: usize,

//     show_fps: bool,

//     screen_width: usize,
//     screen_height: usize,
//     // canvas: sdl2::render::Canvas<sdl2::video::Window>,
//     // surface: sdl2::surface::Surface<'static>,
//     // texture_creator: sdl2::render::TextureCreator<sdl2::video::WindowContext>,
//     // audio_queue: AudioQueue<i16>,
//     // event_pump: EventPump,
//     input_manager: InputManager,
// }

// struct AutoSavedState {
//     thumbnail: Texture,
//     data: Vec<u8>,
// }

// impl App {
//     fn new(rom_file: &Path, boot_rom: &Option<PathBuf>) -> Result<Self> {
//         let rom = load_rom(&rom_file)?;
//         if log_enabled!(log::Level::Info) {
//             print_rom_info(&rom.info());
//         }

//         let backup_ram = load_backup_ram(&rom_file)?;

//         let boot_rom = if let Some(boot_rom) = boot_rom {
//             Some(fs::read(boot_rom)?)
//         } else {
//             None
//         };

//         let config = Config::default()
//             .set_dmg_palette(&DMG_PALETTE)
//             .set_boot_rom(boot_rom);

//         let gb = GameBoy::new(rom, backup_ram, &config)?;

//         let (width, height) = {
//             let buf = gb.frame_buffer();
//             (buf.width, buf.height)
//         };

//         let screen_width = width * SCALING;
//         let screen_height = height * SCALING;

//         let sdl_context = sdl2::init().map_err(|e| anyhow!("{e}"))?;
//         let video_subsystem = sdl_context.video().map_err(|e| anyhow!("{e}"))?;

//         let window = video_subsystem
//             .window("TGB-R", screen_width as u32, screen_height as u32)
//             .build()?;

//         let canvas = window.into_canvas().present_vsync().build()?;
//         let texture_creator = canvas.texture_creator();

//         let surface =
//             sdl2::surface::Surface::new(width as u32, height as u32, PixelFormatEnum::RGB24)
//                 .map_err(|e| anyhow!("{e}"))?;

//         let audio_subsystem = sdl_context.audio().map_err(|e| anyhow!("{e}"))?;
//         let desired_spec = AudioSpecDesired {
//             freq: Some(AUDIO_FREQUENCY as _),
//             channels: Some(2),
//             samples: Some(AUDIO_BUFFER_SAMPLES as _),
//         };
//         let audio_queue: AudioQueue<i16> = audio_subsystem
//             .open_queue(None, &desired_spec)
//             .map_err(|e| anyhow!("{e}"))?;
//         audio_queue
//             .queue_audio(&vec![0; AUDIO_BUFFER_SAMPLES * 2])
//             .map_err(|e| anyhow!("{e}"))?;
//         audio_queue.resume();

//         let key_config = KeyConfig::default();
//         let hotkeys = HotKeys::default();
//         let input_manager = InputManager::new(&sdl_context, &key_config, &hotkeys)?;
//         let event_pump = sdl_context.event_pump().map_err(|e| anyhow!("{e}"))?;

//         Ok(Self {
//             gb,
//             rom_file: rom_file.to_owned(),

//             state: AppState::Running,
//             frames: 0,
//             timer: timer::Timer::new(),
//             state_save_slot: 0,
//             auto_saved_states: VecDeque::new(),
//             rewind_pos: 0,

//             show_fps: true,

//             screen_width,
//             screen_height,
//             // canvas,
//             // surface,
//             // texture_creator,
//             // audio_queue,
//             // event_pump,
//             input_manager,
//         })
//     }

//     fn run(&mut self) -> Result<()> {
//         let ttf_context = sdl2::ttf::init().map_err(|e| anyhow!("{e}"))?;
//         let font = ttf_context
//             .load_font("./assets/fonts/PixelMplus12-Regular.ttf", 36)
//             .map_err(|e| anyhow!("{e}"))?;

//         while process_events(&mut self.event_pump) {
//             self.input_manager.update(&self.event_pump);
//             self.dispatch_event()?;

//             match self.state {
//                 AppState::Running => {
//                     self.running(&font)?;
//                     self.frames += 1;
//                 }
//                 AppState::Rewinding => {
//                     self.rewinding()?;
//                 }
//                 AppState::Paused => todo!(),
//             }
//         }

//         if let Some(ram) = self.gb.backup_ram() {
//             save_backup_ram(&self.rom_file, &ram)?;
//         } else {
//             info!("No backup RAM to save");
//         }

//         Ok(())
//     }

//     fn dispatch_event(&mut self) -> Result<()> {
//         match self.state {
//             AppState::Running => {
//                 if self.input_manager.hotkey(HotKey::Reset).pushed() {
//                     self.gb.reset();
//                     info!("Reset machine");
//                 }

//                 if self.input_manager.hotkey(HotKey::StateSave).pushed() {
//                     let data = self.gb.save_state();
//                     save_state_data(&self.rom_file, self.state_save_slot, &data)?;
//                 }

//                 if self.input_manager.hotkey(HotKey::StateLoad).pushed() {
//                     let data = load_state_data(&self.rom_file, self.state_save_slot)?;
//                     let res = self.gb.load_state(&data);
//                     if let Err(e) = res {
//                         error!("Failed to load state: {}", e);
//                     }
//                 }

//                 if self.input_manager.hotkey(HotKey::NextSlot).pushed() {
//                     self.state_save_slot += 1;
//                     info!("State save slot changed: {}", self.state_save_slot);
//                 }

//                 if self.input_manager.hotkey(HotKey::PrevSlot).pushed() {
//                     self.state_save_slot = self.state_save_slot.saturating_sub(1);
//                     info!("State save slot changed: {}", self.state_save_slot);
//                 }

//                 if self.input_manager.hotkey(HotKey::Rewind).pushed() {
//                     self.auto_state_save()?;
//                     self.state = AppState::Rewinding;
//                     self.rewind_pos = self.auto_saved_states.len() - 1;
//                 }
//             }
//             AppState::Rewinding => {
//                 if self.input_manager.pad_button(PadButton::Left).pushed() {
//                     self.rewind_pos = self.rewind_pos.saturating_sub(1);
//                 }
//                 if self.input_manager.pad_button(PadButton::Right).pushed() {
//                     self.rewind_pos = min(self.auto_saved_states.len() - 1, self.rewind_pos + 1);
//                 }
//                 if self.input_manager.pad_button(PadButton::A).pushed()
//                     || self.input_manager.pad_button(PadButton::Start).pushed()
//                 {
//                     self.gb
//                         .load_state(&self.auto_saved_states[self.rewind_pos].data)?;
//                     while self.auto_saved_states.len() > self.rewind_pos {
//                         let st = self.auto_saved_states.pop_back().unwrap();
//                         unsafe { st.thumbnail.destroy() };
//                     }
//                     self.state = AppState::Running;
//                     self.frames = 0;
//                     info!("State rewinded");
//                 }
//                 if self.input_manager.pad_button(PadButton::B).pushed() {
//                     self.state = AppState::Running;
//                 }
//             }
//             AppState::Paused => todo!(),
//         }

//         Ok(())
//     }

//     fn running(&mut self, font: &sdl2::ttf::Font<'_, '_>) -> Result<()> {
//         let input = self.input_manager.input();
//         self.gb.set_input(&input);
//         self.gb.exec_frame();

//         if self.frames % AUTO_STATE_SAVE_FREQUENCY == 0 {
//             self.auto_state_save()?;
//         }

//         let is_turbo = self.input_manager.hotkey(HotKey::Turbo).pressed();

//         if !is_turbo || self.frames % FRAME_SKIP_ON_TURBO == 0 {
//             let texture = self.to_texture(self.gb.frame_buffer())?;
//             self.canvas
//                 .copy(&texture, None, None)
//                 .map_err(|e| anyhow!("{e}"))?;
//             unsafe { texture.destroy() };

//             if self.show_fps {
//                 self.render_fps(font)?;
//             }
//             self.canvas.present();
//         }

//         if !is_turbo {
//             self.sync_audio();
//         }
//         self.queue_audio()?;

//         let fps = if !is_turbo { 999.9 } else { 999.0 };
//         self.timer.wait_for_frame(fps);
//         Ok(())
//     }

//     fn rewinding(&mut self) -> Result<()> {
//         self.canvas.set_draw_color(Color::RGB(0, 0, 0));
//         self.canvas.clear();

//         self.canvas.set_draw_color(Color::RGB(64, 64, 64));
//         self.canvas
//             .fill_rect(self.convert_coord((0.5, 5.0 / 6.0), 1.0, 1.0 / 3.0))
//             .map_err(|e| anyhow!("{e}"))?;

//         self.canvas
//             .copy(
//                 &self.auto_saved_states[self.rewind_pos].thumbnail,
//                 None,
//                 self.convert_coord((0.5, 1.0 / 3.0), 2.0 / 3.0 * 0.95, 2.0 / 3.0 * 0.95),
//             )
//             .map_err(|e| anyhow!("{e}"))?;

//         self.canvas.set_draw_color(Color::RGB(200, 200, 200));
//         self.canvas
//             .fill_rect(self.convert_coord((0.5, 5.0 / 6.0), 0.2, 0.2))
//             .map_err(|e| anyhow!("{e}"))?;

//         for i in -2..=2 {
//             let ix = self.rewind_pos as isize + i;
//             if !(ix >= 0 && ix < self.auto_saved_states.len() as isize) {
//                 continue;
//             }
//             let ix = ix as usize;
//             let x = 0.5 + (i * 2) as f64 * 0.1;
//             let y = 5.0 / 6.0;
//             let scale = 0.2 * if i == 0 { 0.95 } else { 0.85 };

//             self.canvas
//                 .copy(
//                     &self.auto_saved_states[ix].thumbnail,
//                     None,
//                     self.convert_coord((x, y), scale, scale),
//                 )
//                 .map_err(|e| anyhow!("{e}"))?;
//         }

//         self.canvas.present();
//         self.timer.wait_for_frame(FPS);

//         Ok(())
//     }

//     fn convert_coord(&self, pt: (f64, f64), w: f64, h: f64) -> Option<Rect> {
//         let cx = pt.0;
//         let cy = pt.1;
//         let l = cx - 0.5 * w;
//         let u = cy - 0.5 * h;
//         Some(
//             (
//                 (l * self.screen_width as f64).round() as i32,
//                 (u * self.screen_height as f64).round() as i32,
//                 (w * self.screen_width as f64).round() as u32,
//                 (h * self.screen_height as f64).round() as u32,
//             )
//                 .into(),
//         )
//     }

//     fn auto_state_save(&mut self) -> Result<()> {
//         let state = AutoSavedState {
//             data: self.gb.save_state(),
//             thumbnail: self.to_texture(self.gb.frame_buffer())?,
//         };
//         self.auto_saved_states.push_back(state);
//         if self.auto_saved_states.len() > MAX_AUTO_STATE_SAVES {
//             let st = self.auto_saved_states.pop_front().unwrap();
//             unsafe { st.thumbnail.destroy() };
//         }

//         Ok(())
//     }

//     fn to_texture(&self, frame_buffer: &FrameBuffer) -> Result<Texture> {
//         let mut surface: Surface<'static> = sdl2::surface::Surface::new(
//             frame_buffer.width as u32,
//             frame_buffer.height as u32,
//             PixelFormatEnum::RGB24,
//         )
//         .map_err(|e| anyhow!("{e}"))?;

//         self.copy_to_surface(&mut surface, frame_buffer);
//         let ret = surface
//             .as_texture(&self.texture_creator)
//             .map_err(|e| anyhow!("{e}"))?;
//         Ok(ret)
//     }

//     fn copy_to_surface(&self, surface: &mut Surface, frame_buffer: &FrameBuffer) {
//         surface.with_lock_mut(|r| {
//             for y in 0..frame_buffer.height {
//                 for x in 0..frame_buffer.width {
//                     let ix = y * frame_buffer.width + x;
//                     let p = frame_buffer.get(x, y);
//                     r[ix * 3 + 0] = p.r;
//                     r[ix * 3 + 1] = p.g;
//                     r[ix * 3 + 2] = p.b;
//                 }
//             }
//         });
//     }

//     fn sync_audio(&mut self) {
//         while self.audio_queue.size() as usize >= AUDIO_BUFFER_SAMPLES * 2 * 2 {
//             std::thread::sleep(Duration::from_millis(1));
//         }
//     }

//     fn queue_audio(&mut self) -> Result<()> {
//         if (self.audio_queue.size() as usize) >= AUDIO_BUFFER_SAMPLES * 2 * 2 {
//             return Ok(());
//         }

//         let audio_buf = self.gb.audio_buffer();
//         assert!(
//             (799..=801).contains(&audio_buf.buf.len()),
//             "invalid generated audio length: {}",
//             audio_buf.buf.len()
//         );
//         self.audio_queue
//             .queue_audio(
//                 &audio_buf
//                     .buf
//                     .iter()
//                     .map(|s| [s.right, s.left])
//                     .flatten()
//                     .collect::<Vec<_>>(),
//             )
//             .map_err(|e| anyhow!("{e}"))?;
//         Ok(())
//     }

//     fn render_fps(&mut self, font: &sdl2::ttf::Font<'_, '_>) -> Result<()> {
//         let text = format!("{:5.02}", self.timer.fps());
//         let fps_tex = font
//             .render(&text[0..5])
//             .blended(Color::WHITE)?
//             .as_texture(&self.texture_creator)?;

//         let (w, h) = {
//             let q = fps_tex.query();
//             (q.width, q.height)
//         };

//         let r1 = Rect::new(
//             self.screen_width as i32 - w as i32 * 11 / 10,
//             0,
//             w * 11 / 10,
//             h,
//         );
//         let r2 = Rect::new(self.screen_width as i32 - w as i32, 0, w, h);

//         self.canvas.set_draw_color(Color::RGBA(0, 0, 0, 192));
//         self.canvas.set_blend_mode(sdl2::render::BlendMode::Blend);
//         self.canvas.fill_rect(r1).map_err(|e| anyhow!("{e}"))?;

//         self.canvas
//             .copy(&fps_tex, None, r2)
//             .map_err(|e| anyhow!("{e}"))?;

//         unsafe { fps_tex.destroy() };

//         Ok(())
//     }
// }

// fn process_events(event_pump: &mut EventPump) -> bool {
//     for event in event_pump.poll_iter() {
//         match event {
//             Event::Quit { .. }
//             | Event::KeyDown {
//                 keycode: Some(sdl2::keyboard::Keycode::Escape),
//                 ..
//             } => return false,
//             _ => {}
//         }
//     }
//     true
// }
