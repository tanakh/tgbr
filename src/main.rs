mod config;
mod file;
mod input;
mod menu;
mod rewinding;

use anyhow::Result;
use bevy_easings::EasingsPlugin;
use bevy_tiled_camera::TiledCameraPlugin;
use log::{error, info, log_enabled};
use menu::MenuPlugin;
use rewinding::{AutoSavedState, RewindingPlugin};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use bevy::{
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use bevy_egui::EguiPlugin;
use bevy_kira_audio::{AudioPlugin, AudioStream, AudioStreamPlugin, Frame, StreamedAudio};

use tgbr_core::{AudioBuffer, Config, FrameBuffer, GameBoy, Input as GameBoyInput};

use config::{load_config, load_persistent_state, Palette};
use file::{load_backup_ram, load_rom, print_rom_info, save_backup_ram};
use input::{check_hotkey, gameboy_input_system, HotKey};

use crate::file::{load_state_data, save_state_data};

#[argopt::cmd]
fn main(
    /// Path to Boot ROM
    #[opt(long)]
    boot_rom: Option<PathBuf>,
    /// Path to Cartridge ROM
    rom_file: Option<PathBuf>,
) -> Result<()> {
    let config = load_config()?;

    let mut app = App::new();
    app.insert_resource(WindowDescriptor {
        title: "TGB-R".to_string(),
        resizable: false,
        ..Default::default()
    })
    .insert_resource(ClearColor(Color::rgb(0.0, 0.0, 0.0)))
    .init_resource::<UiState>()
    .insert_resource(Msaa { samples: 4 })
    .add_plugins(DefaultPlugins)
    .add_plugin(TiledCameraPlugin)
    .add_plugin(AudioPlugin)
    .add_plugin(EasingsPlugin)
    .add_plugin(EguiPlugin)
    .add_plugin(MenuPlugin)
    .add_plugin(GameBoyPlugin)
    .add_plugin(RewindingPlugin)
    .add_event::<HotKey>()
    .add_startup_system(setup);

    if let Some(rom_file) = rom_file {
        let gb = GameBoyState::new(
            rom_file,
            config.boot_rom(),
            config.save_dir(),
            config.palette(),
        )?;
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

fn setup(mut commands: Commands) {
    use bevy_tiled_camera::*;
    commands.spawn_bundle(TiledCameraBundle::new().with_target_resolution(1, [160, 144]));
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum AppState {
    Menu,
    Running,
    Rewinding,
}

pub struct GameBoyState {
    gb: GameBoy,
    rom_file: PathBuf,
    save_dir: PathBuf,
    frames: usize,
    pub auto_saved_states: VecDeque<AutoSavedState>,
}

impl GameBoyState {
    fn new(
        rom_file: impl AsRef<Path>,
        boot_rom: Option<Vec<u8>>,
        save_dir: impl AsRef<Path>,
        palette: &Palette,
    ) -> Result<Self> {
        let rom = load_rom(rom_file.as_ref())?;
        if log_enabled!(log::Level::Info) {
            print_rom_info(&rom.info());
        }

        let backup_ram = load_backup_ram(rom_file.as_ref(), save_dir.as_ref())?;

        let config = Config::default()
            .set_dmg_palette(palette)
            .set_boot_rom(boot_rom);

        let gb = GameBoy::new(rom, backup_ram, &config)?;

        Ok(Self {
            gb,
            rom_file: rom_file.as_ref().to_owned(),
            save_dir: save_dir.as_ref().to_owned(),
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
        app.add_plugin(AudioStreamPlugin::<AudioStreamQueue>::default())
            .init_resource::<GameBoyInput>()
            .add_system_set(
                SystemSet::on_update(AppState::Running)
                    .with_system(check_hotkey)
                    .with_system(process_hotkey)
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
struct UiState {
    state_save_slot: usize,
}

#[derive(Component)]
pub struct ScreenSprite;

fn setup_gameboy_system(
    mut commands: Commands,
    windows: ResMut<Windows>,
    config: Res<config::Config>,
    gb_state: Res<GameBoyState>,
    mut images: ResMut<Assets<Image>>,
    audio: Res<StreamedAudio<AudioStreamQueue>>,
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

    let audio_queue = Arc::new(Mutex::new(VecDeque::new()));

    audio.stream(AudioStreamQueue {
        queue: Arc::clone(&audio_queue),
    });

    commands.insert_resource(AudioStreamQueue { queue: audio_queue });

    resume_gameboy_system(windows, config, gb_state);
}

fn resume_gameboy_system(
    mut windows: ResMut<Windows>,
    config: Res<config::Config>,
    gb_state: Res<GameBoyState>,
) {
    let window = windows.get_primary_mut().unwrap();
    let scale = config.scaling() as f32;
    let width = gb_state.gb.frame_buffer().width as u32;
    let height = gb_state.gb.frame_buffer().height as u32;
    window.set_resolution(width as f32 * scale, height as f32 * scale);
}

fn exit_gameboy_system(mut commands: Commands, screen_entity: Query<Entity, With<ScreenSprite>>) {
    let screen_entity = screen_entity.single();
    commands.entity(screen_entity).despawn();
}

fn process_hotkey(
    config: Res<config::Config>,
    mut reader: EventReader<HotKey>,
    mut app_state: ResMut<State<AppState>>,
    mut gb_state: Option<ResMut<GameBoyState>>,
    mut ui_state: ResMut<UiState>,
) {
    for hotkey in reader.iter() {
        match hotkey {
            HotKey::Reset => {
                if let Some(state) = &mut gb_state {
                    state.gb.reset();
                    info!("Reset machine");
                }
            }
            HotKey::StateSave => {
                if let Some(state) = &mut gb_state {
                    let data = state.gb.save_state();
                    save_state_data(
                        &state.rom_file,
                        ui_state.state_save_slot,
                        &data,
                        config.state_dir(),
                    )
                    .unwrap();
                    info!("State saved to slot {}", ui_state.state_save_slot);
                }
            }
            HotKey::StateLoad => {
                if let Some(state) = &mut gb_state {
                    let res = (|| {
                        let data = load_state_data(
                            &state.rom_file,
                            ui_state.state_save_slot,
                            config.state_dir(),
                        )?;
                        state.gb.load_state(&data)
                    })();
                    if let Err(e) = res {
                        error!("Failed to load state: {}", e);
                    }
                }
            }
            HotKey::NextSlot => {
                ui_state.state_save_slot += 1;
                info!("State save slot changed: {}", ui_state.state_save_slot);
            }
            HotKey::PrevSlot => {
                ui_state.state_save_slot = ui_state.state_save_slot.saturating_sub(1);
                info!("State save slot changed: {}", ui_state.state_save_slot);
            }
            HotKey::Rewind => {
                if app_state.current() == &AppState::Running {
                    let gb_state = gb_state.as_mut().unwrap();

                    let saved_state = AutoSavedState {
                        data: gb_state.gb.save_state(),
                        thumbnail: frame_buffer_to_image(gb_state.gb.frame_buffer()),
                    };

                    gb_state.auto_saved_states.push_back(saved_state);
                    if gb_state.auto_saved_states.len() > config.auto_state_save_limit() {
                        gb_state.auto_saved_states.pop_front();
                    }

                    app_state.push(AppState::Rewinding).unwrap();
                }
            }
            HotKey::Menu => {
                app_state.set(AppState::Menu).unwrap();
            }
            HotKey::FullScreen => {}

            HotKey::Turbo => {}
        }
    }
}

fn gameboy_system(
    screen: Res<GameScreen>,
    config: Res<config::Config>,
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

    let mut exec_frame = |queue: &mut VecDeque<Frame>| {
        state.gb.exec_frame();
        if state.frames % config.auto_state_save_freq() == 0 {
            let saved_state = AutoSavedState {
                data: state.gb.save_state(),
                thumbnail: frame_buffer_to_image(state.gb.frame_buffer()),
            };

            state.auto_saved_states.push_back(saved_state);
            if state.auto_saved_states.len() > config.auto_state_save_limit() {
                state.auto_saved_states.pop_front();
            }

            info!("Auto state saved");
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
    copy_frame_buffer(&mut image.data, fb);
}

fn frame_buffer_to_image(frame_buffer: &FrameBuffer) -> Image {
    let width = frame_buffer.width as u32;
    let height = frame_buffer.height as u32;

    let mut data = vec![0; width as usize * height as usize * 4];
    copy_frame_buffer(&mut data, frame_buffer);
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

fn copy_frame_buffer(data: &mut [u8], frame_buffer: &FrameBuffer) {
    let width = frame_buffer.width;
    let height = frame_buffer.height;

    for y in 0..height {
        for x in 0..width {
            let ix = y * width + x;
            let pixel = &mut data[ix * 4..ix * 4 + 4];
            pixel[0] = frame_buffer.buf[ix].r;
            pixel[1] = frame_buffer.buf[ix].g;
            pixel[2] = frame_buffer.buf[ix].b;
            pixel[3] = 0xff;
        }
    }
}

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
