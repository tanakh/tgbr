use anyhow::Result;
use bevy_easings::EasingsPlugin;
use bevy_tiled_camera::TiledCameraPlugin;
use log::{error, info, log_enabled};
use std::{
    cmp::min,
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use bevy::{
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    input::{mouse::MouseButtonInput, ElementState},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    window::WindowMode,
};
use bevy_egui::EguiPlugin;
use bevy_kira_audio::{AudioPlugin, AudioStream, AudioStreamPlugin, Frame, StreamedAudio};

use tgbr_core::{AudioBuffer, Config, FrameBuffer, GameBoy, Input as GameBoyInput};

use crate::{
    config::{self, load_config, load_persistent_state},
    file::{load_backup_ram, load_rom, print_rom_info, save_backup_ram},
    hotkey,
    input::gameboy_input_system,
    menu,
    rewinding::{self, AutoSavedState},
};

pub fn main(boot_rom: Option<PathBuf>, rom_file: Option<PathBuf>) -> Result<()> {
    let config = load_config()?;

    let mut app = App::new();
    app.insert_resource(WindowDescriptor {
        title: "TGB-R".to_string(),
        resizable: false,
        vsync: true,
        ..Default::default()
    })
    .insert_resource(ClearColor(Color::rgb(0.0, 0.0, 0.0)))
    .init_resource::<UiState>()
    .init_resource::<FullscreenState>()
    .insert_resource(Msaa { samples: 4 })
    .insert_resource(bevy::log::LogSettings {
        level: bevy::utils::tracing::Level::INFO,
        filter: "wgpu=error,tgbr_core::cpu=info".to_string(),
    })
    .add_plugins(DefaultPlugins)
    .add_plugin(FrameTimeDiagnosticsPlugin)
    .add_plugin(TiledCameraPlugin)
    .add_plugin(AudioPlugin)
    .add_plugin(AudioStreamPlugin::<AudioStreamQueue>::default())
    .add_plugin(EasingsPlugin)
    .add_plugin(EguiPlugin)
    .add_plugin(hotkey::HotKeyPlugin)
    .add_plugin(menu::MenuPlugin)
    .add_plugin(GameBoyPlugin)
    .add_plugin(rewinding::RewindingPlugin)
    .add_event::<WindowControlEvent>()
    .add_system(window_control_event)
    .insert_resource(LastClicked(0.0))
    .add_system(process_double_click)
    .add_startup_system(setup);

    if let Some(rom_file) = rom_file {
        let gb = GameBoyState::new(rom_file, &config)?;
        app.insert_resource(gb);
        app.add_state(AppState::Running);
    } else {
        app.add_state(AppState::Menu);
    }

    app.insert_resource(config);
    app.insert_resource(load_persistent_state()?);

    app.run();
    Ok(())
}

fn setup(mut commands: Commands, audio: Res<StreamedAudio<AudioStreamQueue>>) {
    use bevy_tiled_camera::*;
    commands.spawn_bundle(TiledCameraBundle::new().with_target_resolution(1, [160, 144]));

    let audio_queue = Arc::new(Mutex::new(VecDeque::new()));

    audio.stream(AudioStreamQueue {
        queue: Arc::clone(&audio_queue),
    });

    commands.insert_resource(AudioStreamQueue { queue: audio_queue });
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum AppState {
    Menu,
    Running,
    Rewinding,
}

pub struct GameBoyState {
    pub gb: GameBoy,
    pub rom_file: PathBuf,
    pub save_dir: PathBuf,
    frames: usize,
    pub auto_saved_states: VecDeque<AutoSavedState>,
}

impl GameBoyState {
    pub fn new(rom_file: impl AsRef<Path>, config: &crate::config::Config) -> Result<Self> {
        let rom = load_rom(rom_file.as_ref())?;
        if log_enabled!(log::Level::Info) {
            print_rom_info(&rom.info());
        }

        let save_dir = config.save_dir().to_owned();
        let backup_ram = load_backup_ram(rom_file.as_ref(), &save_dir)?;

        let config = Config::default()
            .set_model(config.model())
            .set_dmg_palette(config.palette().get_palette())
            .set_boot_rom(config.boot_roms());

        let gb = GameBoy::new(rom, backup_ram, &config)?;

        Ok(Self {
            gb,
            rom_file: rom_file.as_ref().to_owned(),
            save_dir: save_dir,
            frames: 0,
            auto_saved_states: VecDeque::new(),
        })
    }
}

impl Drop for GameBoyState {
    fn drop(&mut self) {
        if let Some(ram) = self.gb.backup_ram() {
            if let Err(err) = save_backup_ram(&self.rom_file, &ram, &self.save_dir) {
                error!("Failed to save backup ram: {err}");
            }
        } else {
            info!("No backup RAM to save");
        }
    }
}

struct GameBoyPlugin;

impl Plugin for GameBoyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameBoyInput>()
            .add_system_set(
                SystemSet::on_update(AppState::Running)
                    .with_system(gameboy_input_system.label("input")),
            )
            .add_system_set(
                SystemSet::on_enter(AppState::Running).with_system(setup_gameboy_system),
            )
            .add_system_set(
                SystemSet::on_resume(AppState::Running).with_system(resume_gameboy_system),
            )
            .add_system_set(
                SystemSet::on_update(AppState::Running)
                    .with_system(gameboy_system)
                    .with_system(fps_system)
                    .after("input"),
            )
            .add_system_set(SystemSet::on_exit(AppState::Running).with_system(exit_gameboy_system));
    }
}

pub struct GameScreen(Handle<Image>);

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

#[derive(Default)]
pub struct UiState {
    pub state_save_slot: usize,
}

#[derive(Component)]
pub struct ScreenSprite;

#[derive(Component)]
pub struct FpsText;

#[derive(Component)]
pub struct FpsTextBg;

fn setup_gameboy_system(
    mut commands: Commands,
    gb_state: Res<GameBoyState>,
    mut images: ResMut<Assets<Image>>,
    mut fonts: ResMut<Assets<Font>>,
    mut event: EventWriter<WindowControlEvent>,
) {
    let width = gb_state.gb.frame_buffer().width as u32;
    let height = gb_state.gb.frame_buffer().height as u32;
    let img = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        vec![0; (width * height * 4) as usize],
        TextureFormat::Rgba8UnormSrgb,
    );

    let texture = images.add(img);
    commands
        .spawn_bundle(SpriteBundle {
            texture: texture.clone(),
            ..Default::default()
        })
        .insert(ScreenSprite);

    commands.insert_resource(GameScreen(texture));

    let fps_font =
        Font::try_from_bytes(include_bytes!("../assets/fonts/PixelMplus12-Regular.ttf").to_vec())
            .unwrap();
    commands
        .spawn_bundle(Text2dBundle {
            text: Text::with_section(
                "",
                TextStyle {
                    font: fonts.add(fps_font),
                    font_size: 24.0,
                    color: Color::WHITE,
                    ..Default::default()
                },
                TextAlignment::default(),
            ),
            transform: Transform::from_xyz(52.0, 72.0, 2.0).with_scale(Vec3::splat(0.5)),
            ..Default::default()
        })
        .insert(FpsText);

    commands
        .spawn_bundle(SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(0.0, 0.0, 0.0, 0.75),
                custom_size: Some(Vec2::new(30.0, 12.0)),
                ..Default::default()
            },
            transform: Transform::from_xyz(65.0, 66.0, 1.0),
            ..Default::default()
        })
        .insert(FpsTextBg);

    event.send(WindowControlEvent::Restore);
}

fn resume_gameboy_system(mut event: EventWriter<WindowControlEvent>) {
    event.send(WindowControlEvent::Restore);
}

fn exit_gameboy_system(
    mut commands: Commands,
    screen_entity: Query<Entity, With<ScreenSprite>>,
    fps_text: Query<Entity, With<FpsText>>,
    fps_text_bg: Query<Entity, With<FpsTextBg>>,
) {
    commands.entity(screen_entity.single()).despawn();
    commands.entity(fps_text.single()).despawn();
    commands.entity(fps_text_bg.single()).despawn();
}

#[derive(Default)]
pub struct FullscreenState(pub bool);

pub enum WindowControlEvent {
    ToggleFullscreen,
    ChangeScale(usize),
    Restore,
}

fn window_control_event(
    mut windows: ResMut<Windows>,
    mut event: EventReader<WindowControlEvent>,
    mut fullscreen_state: ResMut<FullscreenState>,
    mut config: ResMut<config::Config>,
    app_state: Res<State<AppState>>,
) {
    let running = app_state.current() == &AppState::Running;

    for event in event.iter() {
        match event {
            WindowControlEvent::ToggleFullscreen => {
                let window = windows.get_primary_mut().unwrap();
                fullscreen_state.0 = !fullscreen_state.0;

                if fullscreen_state.0 {
                    window.set_mode(WindowMode::BorderlessFullscreen);
                } else {
                    window.set_mode(WindowMode::Windowed);
                }
                if running {
                    let window = windows.get_primary_mut().unwrap();
                    restore_window(window, fullscreen_state.0, config.scaling());
                }
            }
            WindowControlEvent::ChangeScale(scale) => {
                config.set_scaling(*scale);
                if running {
                    let window = windows.get_primary_mut().unwrap();
                    restore_window(window, fullscreen_state.0, config.scaling());
                }
            }
            WindowControlEvent::Restore => {
                let window = windows.get_primary_mut().unwrap();
                restore_window(window, fullscreen_state.0, config.scaling());
            }
        }
    }
}

struct LastClicked(f64);

fn process_double_click(
    time: Res<Time>,
    mut last_clicked: ResMut<LastClicked>,
    mut mouse_button_event: EventReader<MouseButtonInput>,
    mut window_control_event: EventWriter<WindowControlEvent>,
) {
    for ev in mouse_button_event.iter() {
        if ev.button == MouseButton::Left && ev.state == ElementState::Pressed {
            let cur = time.seconds_since_startup();
            let diff = cur - last_clicked.0;

            if diff < 0.25 {
                window_control_event.send(WindowControlEvent::ToggleFullscreen);
            }

            last_clicked.0 = cur;
        }
    }
}

fn restore_window(window: &mut Window, fullscreen: bool, scaling: usize) {
    let width = 160;
    let height = 144;

    if !fullscreen {
        let scale = scaling as f32;
        window.set_resolution(width as f32 * scale, height as f32 * scale);
    }
}

fn gameboy_system(
    screen: Res<GameScreen>,
    config: Res<config::Config>,
    mut state: ResMut<GameBoyState>,
    mut images: ResMut<Assets<Image>>,
    input: Res<GameBoyInput>,
    audio_queue: Res<AudioStreamQueue>,
    is_turbo: Res<hotkey::IsTurbo>,
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

    let cc = make_color_correction(state.gb.model().is_cgb() && config.color_correction());

    if !is_turbo.0 {
        if queue.len() > samples_per_frame * 4 {
            // execution too fast. wait 1 frame.
            return;
        }

        let mut exec_frame = |queue: &mut VecDeque<Frame>| {
            state.gb.exec_frame();
            if state.frames % config.auto_state_save_freq() == 0 {
                let saved_state = AutoSavedState {
                    data: state.gb.save_state(),
                    thumbnail: cc.frame_buffer_to_image(state.gb.frame_buffer()),
                };

                state.auto_saved_states.push_back(saved_state);
                if state.auto_saved_states.len() > config.auto_state_save_limit() {
                    state.auto_saved_states.pop_front();
                }
            }
            push_audio_queue(&mut *queue, state.gb.audio_buffer());
            state.frames += 1;
        };

        if queue.len() < samples_per_frame * 2 {
            // execution too slow. run 2 frame for supply enough audio samples.
            exec_frame(&mut *queue);
        }
        exec_frame(&mut *queue);

        // Update texture
        let fb = state.gb.frame_buffer();
        let image = images.get_mut(&screen.0).unwrap();
        cc.copy_frame_buffer(&mut image.data, fb);
    } else {
        for _ in 0..5 {
            state.gb.exec_frame();
            if queue.len() < samples_per_frame * 2 {
                push_audio_queue(&mut *queue, state.gb.audio_buffer());
            }
        }
        // Update texture
        let fb = state.gb.frame_buffer();
        let image = images.get_mut(&screen.0).unwrap();
        cc.copy_frame_buffer(&mut image.data, fb);
        state.frames += 1;
    }
}

pub fn make_color_correction(color_correction: bool) -> Box<dyn ColorCorrection> {
    if color_correction {
        Box::new(CorrectColor) as Box<dyn ColorCorrection>
    } else {
        Box::new(RawColor) as Box<dyn ColorCorrection>
    }
}

pub trait ColorCorrection {
    fn translate(&self, c: &tgbr_core::Color) -> tgbr_core::Color;

    fn frame_buffer_to_image(&self, frame_buffer: &FrameBuffer) -> Image {
        let width = frame_buffer.width as u32;
        let height = frame_buffer.height as u32;

        let mut data = vec![0; width as usize * height as usize * 4];
        self.copy_frame_buffer(&mut data, frame_buffer);
        Image::new(
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            data,
            TextureFormat::Rgba8UnormSrgb,
        )
    }

    fn copy_frame_buffer(&self, data: &mut [u8], frame_buffer: &FrameBuffer) {
        let width = frame_buffer.width;
        let height = frame_buffer.height;

        for y in 0..height {
            for x in 0..width {
                let ix = y * width + x;
                let pixel = &mut data[ix * 4..ix * 4 + 4];
                let c = self.translate(&frame_buffer.buf[ix]);
                pixel[0] = c.r;
                pixel[1] = c.g;
                pixel[2] = c.b;
                pixel[3] = 0xff;
            }
        }
    }
}

struct RawColor;

impl ColorCorrection for RawColor {
    fn translate(&self, c: &tgbr_core::Color) -> tgbr_core::Color {
        c.clone()
    }
}

struct CorrectColor;

impl ColorCorrection for CorrectColor {
    fn translate(&self, c: &tgbr_core::Color) -> tgbr_core::Color {
        let r = c.r as u16;
        let g = c.g as u16;
        let b = c.b as u16;
        tgbr_core::Color {
            r: min(240, ((r * 26 + g * 4 + b * 2) / 32) as u8),
            g: min(240, ((g * 24 + b * 8) / 32) as u8),
            b: min(240, ((r * 6 + g * 4 + b * 22) / 32) as u8),
        }
    }
}

fn fps_system(
    config: Res<config::Config>,
    diagnostics: ResMut<Diagnostics>,
    is_turbo: Res<hotkey::IsTurbo>,
    mut q: QuerySet<(
        QueryState<(&mut Text, &mut Visibility), With<FpsText>>,
        QueryState<&mut Visibility, With<FpsTextBg>>,
    )>,
) {
    let mut q0 = q.q0();
    let (mut text, mut visibility) = q0.single_mut();
    visibility.is_visible = config.show_fps();
    let fps_diag = diagnostics.get(FrameTimeDiagnosticsPlugin::FPS).unwrap();
    let fps = fps_diag.value().unwrap_or(0.0) * if is_turbo.0 { 5.0 } else { 1.0 };
    let fps = format!("{fps:5.02}");
    text.sections[0].value = fps.chars().take(5).collect();

    let mut q1 = q.q1();
    let mut visibility_bg = q1.single_mut();
    visibility_bg.is_visible = config.show_fps();
}
