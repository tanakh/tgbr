use std::path::{Path, PathBuf};

use crate::{
    config::{Config, PersistentState},
    AppState, GameBoyState, ScreenSprite,
};
use bevy::{app::AppExit, prelude::*};
use bevy_egui::{egui, EguiContext};

pub struct MenuPlugin;

enum MenuEvent {
    OpenRomFile(PathBuf),
}

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_system_set(SystemSet::on_enter(AppState::Menu).with_system(menu_setup))
            .add_system_set(
                SystemSet::on_update(AppState::Menu)
                    .with_system(menu_system)
                    .with_system(menu_event_system),
            )
            .add_event::<MenuEvent>();
    }
}

fn menu_setup(mut windows: ResMut<Windows>, mut egui_ctx: ResMut<EguiContext>) {
    let window = windows.get_primary_mut().unwrap();
    window.set_resolution(960.0, 540.0);

    let mut fonts = egui::FontDefinitions::default();

    // // Install my own font (maybe supporting non-latin characters):
    // fonts.font_data.insert(
    //     "my_font".to_owned(),
    //     egui::FontData::from_static(include_bytes!("../assets/fonts/PixelMplus12-Regular.ttf")),
    // ); // .ttf and .otf supported

    fonts.family_and_size.insert(
        egui::TextStyle::Button,
        (egui::FontFamily::Proportional, 24.0),
    );

    egui_ctx.ctx_mut().set_fonts(fonts);
}

fn menu_event_system(
    mut commands: Commands,
    mut event: EventReader<MenuEvent>,
    mut app_state: ResMut<State<AppState>>,
    mut persistent_state: ResMut<PersistentState>,
    config: Res<Config>,
) {
    for event in event.iter() {
        match event {
            MenuEvent::OpenRomFile(path) => {
                info!("Opening file: {:?}", path);
                match GameBoyState::new(
                    &path,
                    None as Option<&Path>,
                    config.save_dir(),
                    config.palette(),
                ) {
                    Ok(gb) => {
                        commands.insert_resource(gb);
                        persistent_state.add_recent(&path);
                        app_state.set(AppState::Running).unwrap();
                    }
                    Err(err) => {
                        error!("{err}");
                    }
                }
            }
        }
    }
}

enum MenuTab {
    File,
    Setting,
}

fn menu_system(
    persistent_state: Res<PersistentState>,
    mut egui_ctx: ResMut<EguiContext>,
    mut app_state: ResMut<State<AppState>>,
    gb_state: Option<Res<GameBoyState>>,
    mut exit: EventWriter<AppExit>,
    mut menu_event: EventWriter<MenuEvent>,
) {
    egui::CentralPanel::default().show(egui_ctx.ctx_mut(), |ui| {
        let width = ui.available_width();
        egui::SidePanel::left("left_panel")
            .frame(egui::Frame::default())
            .show_inside(ui, |ui| {
                ui.set_width(width / 4.0);
                if ui.button("Setting").clicked() {
                    todo!();
                }
                if ui.button("Quit").clicked() {
                    exit.send(AppExit);
                }
            });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.with_layout(egui::Layout::default().with_cross_justify(true), |ui| {
                if gb_state.is_some() {
                    if ui.button("Resume").clicked() {
                        app_state.set(AppState::Running).unwrap();
                    }
                }

                if ui.button("Load ROM").clicked() {
                    let file = rfd::FileDialog::new()
                        .add_filter("GameBoy ROM file", &["gb", "gbc", "zip"])
                        .pick_file();
                    if let Some(file) = file {
                        menu_event.send(MenuEvent::OpenRomFile(file));
                    }
                }

                ui.separator();
                ui.label("Recent Files");

                for recent in &persistent_state.recent {
                    if ui
                        .button(recent.file_name().unwrap().to_string_lossy().to_string())
                        .clicked()
                    {
                        menu_event.send(MenuEvent::OpenRomFile(recent.clone()));
                    }
                }
            })
        });
    });

    // let flip_fullscreen = |windows: &mut ResMut<Windows>| {
    //     let window = windows.get_primary_mut().unwrap();
    //     let cur_mode = window.mode();
    //     match cur_mode {
    //         WindowMode::Windowed => window.set_mode(WindowMode::BorderlessFullscreen),
    //         WindowMode::BorderlessFullscreen => window.set_mode(WindowMode::Windowed),
    //         _ => unreachable!(),
    //     }
    // };

    // let mut set_window_scale = |windows: &mut ResMut<Windows>, scale: usize| {
    //     let window = windows.get_primary_mut().unwrap();
    //     let (w, h) = calc_window_size(scale);

    //     window.set_resolution(w, h);

    //     // for mut trans in screen_trans.iter_mut() {
    //     //     *trans = Transform::from_scale(Vec3::new(scale as f32, scale as f32, 1.0))
    //     //         .with_translation(Vec3::new(0.0, -((MENU_HEIGHT / 2) as f32), 0.0));
    //     // }
    // };

    // egui::TopBottomPanel::top("top_panel").show(egui_ctx.ctx_mut(), |ui| {
    //     egui::menu::bar(ui, |ui| {
    //         egui::menu::menu_button(ui, "File", |ui| {
    //             ui.menu_button("Open Recent", |ui| {
    //                 ui.set_width_range(150.0..=300.0);
    //                 for recent_file in &persistent_state.recent {
    //                     let text = recent_file.file_name().unwrap().to_str().unwrap();
    //                     let text = if text.chars().count() > 32 {
    //                         format!("{}...", &text.chars().take(32).collect::<String>())
    //                     } else {
    //                         text.to_string()
    //                     };
    //                     if ui.button(text).clicked() {
    //                         ui.close_menu();
    //                         load_rom_file = Some(recent_file.to_owned());
    //                     }
    //                 }
    //             });
    //             ui.separator();
    //             if ui.button("Quit").clicked() {
    //                 exit.send(AppExit);
    //             }
    //         });
    //         egui::menu::menu_button(ui, "Option", |ui| {
    //             if ui.button("Fullscreen").clicked() {
    //                 flip_fullscreen(&mut windows);
    //                 ui.close_menu();
    //             }
    //             ui.menu_button("Scale", |ui| {
    //                 for i in 1..=8 {
    //                     if ui.button(format!("{}x", i)).clicked() {
    //                         ui.close_menu();
    //                         set_window_scale(&mut windows, i);
    //                         config.set_scaling(i);
    //                     }
    //                 }
    //             });
    //         });
    //     });
    // });
}
