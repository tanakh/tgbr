use bevy::prelude::*;
use tgbr_core::Input as GameBoyInput;

use crate::{config, input::InputState, GameBoyState, ScreenSprite};

pub struct AutoSavedState {
    pub thumbnail: Image,
    pub data: Vec<u8>,
}

pub struct RewindingState {
    pos: usize,
    preview_image: Handle<Image>,
}

#[derive(Component)]
pub struct Preview;

#[derive(Component)]
pub struct Thumbnail;

pub fn enter_rewinding_system(
    mut commands: Commands,
    gb_state: ResMut<GameBoyState>,
    mut images: ResMut<Assets<Image>>,
    mut screen_visibility: Query<&mut Visibility, With<ScreenSprite>>,
) {
    for mut visibility in screen_visibility.iter_mut() {
        visibility.is_visible = false;
    }

    let preview_image = images.add(gb_state.auto_saved_states.get(0).unwrap().thumbnail.clone());

    commands
        .spawn_bundle(SpriteBundle {
            texture: preview_image.clone(),
            ..Default::default()
        })
        .insert(Preview);

    for _ in 0..7 {
        commands
            .spawn_bundle(SpriteBundle {
                ..Default::default()
            })
            .insert(Thumbnail);
    }

    commands.insert_resource(RewindingState {
        pos: 0,
        preview_image,
    });
}

pub fn rewinding_system(
    mut commands: Commands,
    mut gb_state: ResMut<GameBoyState>,
    mut rewinding_state: ResMut<RewindingState>,
    preview: Query<&Handle<Image>, With<Preview>>,
    thumbnails: Query<&Transform, With<Thumbnail>>,
    config: Res<config::Config>,
    input_keycode: Res<Input<KeyCode>>,
    input_gamepad_button: Res<Input<GamepadButton>>,
    input_gamepad_axis: Res<Axis<GamepadAxis>>,
) {
    let input_state = InputState::new(&input_keycode, &input_gamepad_button, &input_gamepad_axis);

    if config.key_config().a.just_pressed(&input_state) {
        todo!();
    }

    if config.key_config().b.just_pressed(&input_state) {
        todo!();
    }
}
