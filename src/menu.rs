use std::{path::PathBuf, time::SystemTime};

use crate::{
    app::{AppState, FullscreenState, GameBoyState, WindowControlEvent},
    config::{Config, PaletteSelect, PersistentState},
    input::KeyConfig,
    key_assign::ToStringKey,
};
use bevy::{app::AppExit, prelude::*};
use bevy_egui::{
    egui::{self, SelectableLabel},
    EguiContext,
};
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
            .add_system_set(
                SystemSet::on_exit(AppState::Menu)
                    .with_system(menu_exit)
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

    commands.insert_resource(MenuState::default());
}

fn menu_exit(config: Res<Config>) {
    config.save().unwrap();
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

#[derive(PartialEq, Eq)]
enum MenuTab {
    File,
    GeneralSetting,
    Graphics,
    Controller,
}

#[derive(PartialEq, Eq)]
enum ControllerTab {
    Keyboard,
    Gamepad,
}

struct MenuState {
    tab: MenuTab,
    controller_tab: ControllerTab,
    controller_button_ix: usize,
    last_palette_changed: Option<SystemTime>,
}

impl Default for MenuState {
    fn default() -> Self {
        MenuState {
            tab: MenuTab::File,
            controller_tab: ControllerTab::Keyboard,
            controller_button_ix: 0,
            last_palette_changed: None,
        }
    }
}

fn menu_system(
    mut config: ResMut<Config>,
    persistent_state: Res<PersistentState>,
    mut egui_ctx: ResMut<EguiContext>,
    mut app_state: ResMut<State<AppState>>,
    mut menu_state: ResMut<MenuState>,
    mut gb_state: Option<ResMut<GameBoyState>>,
    mut exit: EventWriter<AppExit>,
    mut menu_event: EventWriter<MenuEvent>,
    mut window_control_event: EventWriter<WindowControlEvent>,
    key_code_input: Res<Input<KeyCode>>,
    gamepad_button_input: Res<Input<GamepadButton>>,
    fullscreen_state: Res<FullscreenState>,
) {
    let MenuState {
        tab,
        controller_tab,
        controller_button_ix,
        last_palette_changed,
    } = menu_state.as_mut();

    if let Some(changed) = last_palette_changed {
        if changed.elapsed().unwrap().as_secs_f64() > 5.0 {
            config.save().unwrap();
            *last_palette_changed = None;
        }
    }

    egui::CentralPanel::default().show(egui_ctx.ctx_mut(), |ui| {
        let width = ui.available_width();
        egui::SidePanel::left("left_panel").show_inside(ui, |ui| {
            ui.set_width(width / 4.0);
            ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                ui.selectable_value(tab, MenuTab::File, "📁File");
                ui.selectable_value(tab, MenuTab::GeneralSetting, "🔧General Setting");
                ui.selectable_value(tab, MenuTab::Graphics, "🖼Graphics");
                ui.selectable_value(tab, MenuTab::Controller, "🎮Controller");
                if ui.button("↩Quit").clicked() {
                    exit.send(AppExit);
                }
            });
        });

        egui::CentralPanel::default().show_inside(ui, |ui| match *tab {
            MenuTab::File => {
                egui::ScrollArea::vertical().show(ui, |ui| {
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
                });
            }
            MenuTab::GeneralSetting => {
                ui.heading("General Settings");
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
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
                });
            }
            MenuTab::Graphics => {
                ui.heading("Gaphics Settings");
                ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| {
                    ui.group(|ui| {
                        let mut color_correction = config.color_correction();
                        if ui
                            .checkbox(&mut color_correction, "Color Correction")
                            .changed()
                        {
                            config.set_color_correction(color_correction);
                        }

                        let mut show_fps = config.show_fps();
                        if ui.checkbox(&mut show_fps, "Display FPS").changed() {
                            config.set_show_fps(show_fps);
                        }

                        let mut fullscreen = fullscreen_state.0;
                        if ui.checkbox(&mut fullscreen, "FullScreen").changed() {
                            window_control_event.send(WindowControlEvent::ToggleFullscreen);
                        }

                        ui.horizontal(|ui| {
                            ui.label("Window Scale:");

                            let mut scale = config.scaling();
                            if ui.add(egui::Slider::new(&mut scale, 1..=8)).changed() {
                                window_control_event
                                    .send(WindowControlEvent::ChangeScale(scale));
                            }
                        });

                        ui.label("GameBoy Palette:");

                        let mut pal: PaletteSelect = config.palette().clone();

                        ui.horizontal(|ui| {
                            egui::ComboBox::from_label("")
                                .width(250.0)
                                .selected_text(match pal {
                                    PaletteSelect::Dmg => "GameBoy",
                                    PaletteSelect::Pocket => "GameBoy Pocket",
                                    PaletteSelect::Light => "GameBoy Light",
                                    PaletteSelect::Custom(_) => "Custom",
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut pal, PaletteSelect::Dmg, "GameBoy");
                                    ui.selectable_value(&mut pal, PaletteSelect::Pocket, "GameBoy Pocket");
                                    ui.selectable_value(&mut pal, PaletteSelect::Light, "GameBoy Light");
                                    if ui.add(SelectableLabel::new(matches!(pal, PaletteSelect::Custom(_)), "Custom")).clicked() {
                                        pal = PaletteSelect::Custom(pal.get_palette().clone());
                                    }
                                }
                            );

                            let cols = pal.get_palette().clone();

                            for i in (0..4).rev() {
                                let mut col = [cols[i].r, cols[i].g, cols[i].b];
                                ui.color_edit_button_srgb(&mut col).changed();
                                
                                if let PaletteSelect::Custom(r) = &mut pal {
                                    r[i] = tgbr_core::Color::new(col[0], col[1], col[2]);
                                }
                            }
                        });

                        if &pal != config.palette() {
                            if let Some(gb_state) = gb_state.as_mut() {
                                gb_state.gb.set_dmg_palette(pal.get_palette());
                            }
                            config.set_palette(pal);
                            *last_palette_changed = Some(SystemTime::now());
                        }
                    });
                });
            }
            MenuTab::Controller => {
                ui.heading("Controller Settings");

                ui.horizontal(|ui| {
                    let mut resp = ui.selectable_value(controller_tab, ControllerTab::Keyboard, "Keyboard");
                    resp |= ui.selectable_value(controller_tab, ControllerTab::Gamepad, "Gamepad");
                    if resp.clicked() {
                        *controller_button_ix = 0;
                    }
                });

                ui.group(|ui| {
                    egui::Grid::new("key_config")
                        .num_columns(2)
                        .spacing([40.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("Button");
                            ui.label("Assignment");
                            ui.end_row();

                            ui.separator();
                            ui.separator();
                            ui.end_row();

                            let mut changed: Option<usize> = None;

                            match *controller_tab {
                                ControllerTab::Keyboard => {
                                    macro_rules! button {
                                        {$ix:literal, $button:ident, $label:literal} => {
                                            ui.label($label);
                                            let assign = config.key_config().$button.extract_keycode()
                                                .map_or_else(|| "".to_string(), |k| format!("{k:?}"));

                                            ui.selectable_value(controller_button_ix, $ix, assign)
                                                .on_hover_text("Click and type the key you want to assign");

                                            if *controller_button_ix == $ix {
                                                if let Some(kc) = key_code_input.get_just_pressed().nth(0) {
                                                    config.key_config_mut().$button.insert_keycode(*kc);
                                                    config.save().unwrap();
                                                    changed = Some($ix);
                                                }
                                            }

                                            ui.end_row();
                                        }
                                    }

                                    button!(1, up, "⏶");
                                    button!(2, down, "⏷");
                                    button!(3, left, "⏴");
                                    button!(4, right, "⏵");
                                    button!(5, a, "A");
                                    button!(6, b, "B");
                                    button!(7, start, "start");
                                    button!(8, select, "select");
                                }

                                ControllerTab::Gamepad => {
                                    macro_rules! button {
                                        {$ix:literal, $button:ident, $label:literal} => {
                                            ui.label($label);
                                            let assign = config.key_config().$button.extract_gamepad()
                                                .map_or_else(|| "".to_string(), |k| ToStringKey(k).to_string());

                                            ui.selectable_value(controller_button_ix, $ix, assign)
                                                .on_hover_text("Click and type the key you want to assign");

                                            if *controller_button_ix == $ix {
                                                if let Some(button) = gamepad_button_input.get_just_pressed().nth(0) {
                                                    config.key_config_mut().$button.insert_gamepad(*button);
                                                    config.save().unwrap();
                                                    changed = Some($ix);
                                                }
                                            }

                                            ui.end_row();
                                        }
                                    }

                                    button!(1, up, "⏶");
                                    button!(2, down, "⏷");
                                    button!(3, left, "⏴");
                                    button!(4, right, "⏵");
                                    button!(5, a, "A");
                                    button!(6, b, "B");
                                    button!(7, start, "start");
                                    button!(8, select, "select");
                                }
                            }

                            if let Some(ix) = changed {
                                *controller_button_ix = ix + 1;
                            }

                        });
                });

                if ui.button("Reset to default").clicked() {
                    let key_config = KeyConfig::default();
                    match *controller_tab {
                        ControllerTab::Keyboard => {
                            macro_rules! button {
                                {$key:ident} => {
                                    let kc = key_config.$key.extract_keycode().unwrap();
                                    config.key_config_mut().$key.insert_keycode(kc);
                                }
                            }
                            button!(up);
                            button!(down);
                            button!(left);
                            button!(right);
                            button!(a);
                            button!(b);
                            button!(start);
                            button!(select);
                        }
                        ControllerTab::Gamepad => {
                            macro_rules! button {
                                {$key:ident} => {
                                    let button = key_config.$key.extract_gamepad().unwrap();
                                    config.key_config_mut().$key.insert_gamepad(button);
                                }
                            }
                            button!(up);
                            button!(down);
                            button!(left);
                            button!(right);
                            button!(a);
                            button!(b);
                            button!(start);
                            button!(select);
                        },
                    }
                    *controller_button_ix = 0;
                    config.save().unwrap();
                }
            }
        });
    });
}
