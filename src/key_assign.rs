use bevy::{input::prelude::*, prelude::KeyCode};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum KeyAssign {
    KeyCode(KeyCode),
    GamepadButton(GamepadButton),
    GamepadAxis(GamepadAxis, GamepadAxisDir),
    All(Vec<KeyAssign>),
    Any(Vec<KeyAssign>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GamepadAxisDir {
    Pos,
    Neg,
}

pub struct ToStringKey<T>(pub T);

impl Display for ToStringKey<KeyCode> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl Display for ToStringKey<GamepadButton> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let GamepadButton(gamepad, button) = &self.0;
        write!(f, "Pad{}.{:?}", gamepad.0, button)
    }
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

    pub fn extract_keycode(&self) -> Option<KeyCode> {
        match self {
            KeyAssign::KeyCode(keycode) => Some(*keycode),
            KeyAssign::Any(ks) => {
                for k in ks {
                    if let Some(keycode) = k.extract_keycode() {
                        return Some(keycode);
                    }
                }
                None
            }
            _ => None,
        }
    }

    pub fn insert_keycode(&mut self, kc: KeyCode) {
        if !self.try_insert_keycode(kc) {
            *self = KeyAssign::Any(vec![KeyAssign::KeyCode(kc), self.clone()]);
        }
    }

    fn try_insert_keycode(&mut self, kc: KeyCode) -> bool {
        match self {
            KeyAssign::KeyCode(r) => {
                *r = kc;
                true
            }
            KeyAssign::Any(ks) => {
                for k in ks {
                    if k.try_insert_keycode(kc) {
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    pub fn extract_gamepad(&self) -> Option<GamepadButton> {
        match self {
            KeyAssign::GamepadButton(button) => Some(*button),
            KeyAssign::Any(ks) => {
                for k in ks {
                    if let Some(button) = k.extract_gamepad() {
                        return Some(button);
                    }
                }
                None
            }
            _ => None,
        }
    }

    pub fn insert_gamepad(&mut self, button: GamepadButton) {
        if !self.try_insert_gamepad(button) {
            *self = KeyAssign::Any(vec![KeyAssign::GamepadButton(button), self.clone()]);
        }
    }

    fn try_insert_gamepad(&mut self, button: GamepadButton) -> bool {
        match self {
            KeyAssign::GamepadButton(r) => {
                *r = button;
                true
            }
            KeyAssign::Any(ks) => {
                for k in ks {
                    if k.try_insert_gamepad(button) {
                        return true;
                    }
                }
                false
            }
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
