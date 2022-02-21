use bevy::{input::prelude::*, prelude::KeyCode};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub enum KeyAssign {
    KeyCode(KeyCode),
    GamepadButton(GamepadButton),
    GamepadAxis(GamepadAxis, GamepadAxisDir),
    All(Vec<KeyAssign>),
    Any(Vec<KeyAssign>),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum GamepadAxisDir {
    Pos,
    Neg,
}

impl KeyAssign {
    pub fn pressed(&self, input_state: &InputState<'_>) -> bool {
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

    pub fn just_pressed(&self, input_state: &InputState<'_>) -> bool {
        match self {
            KeyAssign::KeyCode(keycode) => input_state.input_keycode.just_pressed(*keycode),
            KeyAssign::GamepadButton(button) => {
                input_state.input_gamepad_button.just_pressed(*button)
            }
            KeyAssign::All(ks) => {
                ks.iter().all(|k| k.pressed(input_state))
                    && ks.iter().any(|k| k.just_pressed(input_state))
            }
            KeyAssign::Any(ks) => ks.iter().any(|k| k.just_pressed(input_state)),
            _ => false,
        }
    }
}

macro_rules! keycode {
    ($code:ident) => {
        KeyAssign::KeyCode(KeyCode::$code)
    };
}
pub(crate) use keycode;

macro_rules! pad_button {
    ($id:literal, $button:ident) => {
        KeyAssign::GamepadButton(GamepadButton(Gamepad($id), GamepadButtonType::$button))
    };
}
pub(crate) use pad_button;

macro_rules! any {
    ($($assign:expr),* $(,)?) => {
        KeyAssign::Any(vec![$($assign),*])
    };
}
pub(crate) use any;

macro_rules! all {
    ($($assign:expr),* $(,)?) => {
        KeyAssign::All(vec![$($assign),*])
    };
}
pub(crate) use all;

pub struct InputState<'a> {
    input_keycode: &'a Input<KeyCode>,
    input_gamepad_button: &'a Input<GamepadButton>,
    input_gamepad_axis: &'a Axis<GamepadAxis>,
}

impl<'a> InputState<'a> {
    pub fn new(
        input_keycode: &'a Input<KeyCode>,
        input_gamepad_button: &'a Input<GamepadButton>,
        input_gamepad_axis: &'a Axis<GamepadAxis>,
    ) -> Self {
        Self {
            input_keycode,
            input_gamepad_button,
            input_gamepad_axis,
        }
    }
}
