use std::path::PathBuf;

use crate::{
    config::{Config, PersistentState},
    AppState, FullscreenState, GameBoyState, WindowControlEvent,
};
use bevy::{app::AppExit, prelude::*};
use bevy_egui::{egui, EguiContext};
use tgbr_core::Model;

pub struct MenuPlugin;

pub enum MenuEvent {
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

fn menu_setup(
    mut commands: Commands,
    mut windows: ResMut<Windows>,
    mut egui_ctx: ResMut<EguiContext>,
    fullscreen_state: Res<FullscreenState>,
) {
    if !fullscreen_state.0 {
        let window = windows.get_primary_mut().unwrap();
        window.set_resolution(960.0, 540.0);
    }

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
    fonts.family_and_size.insert(
        egui::TextStyle::Body,
        (egui::FontFamily::Proportional, 24.0),
    );

    fonts.family_and_size.insert(
        egui::TextStyle::Heading,
        (egui::FontFamily::Proportional, 32.0),
    );

    egui_ctx.ctx_mut().set_fonts(fonts);

    commands.insert_resource(MenuTab::File);
}

fn menu_event_system(
    mut commands: Commands,
    mut event: EventReader<MenuEvent>,
    mut app_state: ResMut<State<AppState>>,
    mut persistent_state: ResMut<PersistentState>,
    config: ResMut<Config>,
) {
    for event in event.iter() {
        match event {
            MenuEvent::OpenRomFile(path) => {
                info!("Opening file: {:?}", path);
                match GameBoyState::new(&path, &config) {
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
    mut config: ResMut<Config>,
    persistent_state: Res<PersistentState>,
    mut egui_ctx: ResMut<EguiContext>,
    mut app_state: ResMut<State<AppState>>,
    mut menu_tab: ResMut<MenuTab>,
    gb_state: Option<Res<GameBoyState>>,
    mut exit: EventWriter<AppExit>,
    mut menu_event: EventWriter<MenuEvent>,
    mut window_control_event: EventWriter<WindowControlEvent>,
    fullscreen_state: Res<FullscreenState>,
) {
    egui::CentralPanel::default().show(egui_ctx.ctx_mut(), |ui| {
        let width = ui.available_width();
        egui::SidePanel::left("left_panel").show_inside(ui, |ui| {
            ui.set_width(width / 4.0);
            ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                ui.label("");
                if ui.button("📁File").clicked() {
                    *menu_tab = MenuTab::File;
                }
                if ui.button("🔧Setting").clicked() {
                    *menu_tab = MenuTab::Setting;
                }
                if ui.button("↩Quit").clicked() {
                    exit.send(AppExit);
                }
            });
        });

        egui::CentralPanel::default().show_inside(ui, |ui| match *menu_tab {
            MenuTab::File => {
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                    if gb_state.is_some() {
                        if ui.button("Resume").clicked() {
                            app_state.set(AppState::Running).unwrap();
                        }
                        ui.separator();
                    }

                    ui.label("Load ROM");
                    if ui.button("Open File").clicked() {
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
                });
            }
            MenuTab::Setting => {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                        ui.heading("General Settings");
                        ui.group(|ui| {
                            ui.label("Model");
                            ui.horizontal(|ui| {
                                let mut val = config.model();
                                ui.radio_value(&mut val, Model::Auto, "Auto");
                                ui.radio_value(&mut val, Model::Dmg, "GameBoy");
                                ui.radio_value(&mut val, Model::Cgb, "GameBoy Color");
                                if config.model() != val {
                                    config.set_model(val);
                                }
                            });

                            ui.horizontal(|ui| {
                                ui.label("Save file directory");
                                if ui.button("Change").clicked() {
                                    let dir = rfd::FileDialog::new()
                                        .set_directory(config.save_dir())
                                        .pick_folder();
                                    if let Some(dir) = dir {
                                        config.set_save_dir(dir);
                                    }
                                }
                            });
                            let s = config.save_dir().display().to_string();
                            ui.add(egui::TextEdit::singleline(&mut s.as_ref()));

                            ui.horizontal(|ui| {
                                ui.label("State save directory");
                                if ui.button("Change").clicked() {
                                    let dir = rfd::FileDialog::new()
                                        .set_directory(config.state_dir())
                                        .pick_folder();
                                    if let Some(dir) = dir {
                                        config.set_state_dir(dir);
                                    }
                                }
                            });
                            let s = config.save_dir().display().to_string();
                            ui.add(egui::TextEdit::singleline(&mut s.as_ref()));

                            // ui.label("Boot ROM (TODO)");
                            // ui.radio(false, "Do not use boot ROM");
                            // ui.radio(false, "Use internal boot ROM");
                            // ui.radio(false, "Use specified boot ROM file");
                        });

                        ui.heading("Graphics");
                        ui.group(|ui| {
                            ui.label("Palette");
                            ui.label("UNDERCONSTRUCTIONS");

                            let mut show_fps = config.show_fps();
                            if ui.checkbox(&mut show_fps, "Display FPS").changed() {
                                config.set_show_fps(show_fps);
                            }

                            let mut fullscreen = fullscreen_state.0;
                            if ui.checkbox(&mut fullscreen, "FullScreen").changed() {
                                window_control_event.send(WindowControlEvent::ToggleFullscreen);
                            }

                            ui.horizontal(|ui| {
                                ui.label("Window Scale");

                                let mut scale = config.scaling();
                                if ui.add(egui::Slider::new(&mut scale, 1..=8)).changed() {
                                    window_control_event
                                        .send(WindowControlEvent::ChangeScale(scale));
                                }
                            });
                        });

                        ui.heading("Audio (TODO)");

                        ui.heading("Key Config (TODO)");
                    });
                });
            }
        });
    });
}
