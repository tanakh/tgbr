use bevy::{input::prelude::*, prelude::KeyCode};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyAssign(pub Vec<MultiKey>);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MultiKey(pub Vec<SingleKey>);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SingleKey {
    KeyCode(KeyCode),
    GamepadButton(GamepadButton),
    GamepadAxis(GamepadAxis, GamepadAxisDir),
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
    pub fn and(self, rhs: Self) -> Self {
        let mut ret = vec![];
        for l in self.0.into_iter() {
            for r in rhs.0.iter() {
                let mut t = l.0.clone();
                t.append(&mut r.0.clone());
                ret.push(MultiKey(t));
            }
        }
        Self(ret)
    }

    pub fn or(mut self, mut rhs: Self) -> Self {
        self.0.append(&mut rhs.0);
        self
    }

    pub fn pressed(&self, input_state: &InputState<'_>) -> bool {
        self.0
            .iter()
            .any(|multi_key| multi_key.pressed(input_state))
    }

    pub fn just_pressed(&self, input_state: &InputState<'_>) -> bool {
        self.0
            .iter()
            .any(|multi_key| multi_key.just_pressed(input_state))
    }

    pub fn extract_keycode(&self) -> Option<KeyCode> {
        for MultiKey(mk) in &self.0 {
            match &mk[..] {
                [SingleKey::KeyCode(r)] => return Some(*r),
                _ => {}
            }
        }
        None
    }

    pub fn insert_keycode(&mut self, kc: KeyCode) {
        for MultiKey(mk) in self.0.iter_mut() {
            match &mut mk[..] {
                [SingleKey::KeyCode(r)] => {
                    *r = kc;
                    return;
                }
                _ => {}
            }
        }
        self.0.push(MultiKey(vec![SingleKey::KeyCode(kc)]));
    }

    pub fn extract_gamepad(&self) -> Option<GamepadButton> {
        for MultiKey(mk) in &self.0 {
            match &mk[..] {
                [SingleKey::GamepadButton(r)] => return Some(*r),
                _ => {}
            }
        }
        None
    }

    pub fn insert_gamepad(&mut self, button: GamepadButton) {
        for MultiKey(mk) in self.0.iter_mut() {
            match &mut mk[..] {
                [SingleKey::GamepadButton(r)] => {
                    *r = button;
                    return;
                }
                _ => {}
            }
        }
        self.0
            .push(MultiKey(vec![SingleKey::GamepadButton(button)]));
    }
}

impl MultiKey {
    fn pressed(&self, input_state: &InputState<'_>) -> bool {
        self.0
            .iter()
            .all(|single_key| single_key.pressed(input_state))
    }

    fn just_pressed(&self, input_state: &InputState<'_>) -> bool {
        self.0
            .iter()
            .all(|single_key| single_key.just_pressed(input_state))
    }
}

impl SingleKey {
    fn pressed(&self, input_state: &InputState<'_>) -> bool {
        match self {
            SingleKey::KeyCode(keycode) => input_state.input_keycode.pressed(*keycode),
            SingleKey::GamepadButton(button) => input_state.input_gamepad_button.pressed(*button),
            SingleKey::GamepadAxis(axis, dir) => {
                input_state
                    .input_gamepad_axis
                    .get(*axis)
                    .map_or(false, |r| match dir {
                        GamepadAxisDir::Pos => r >= 0.5,
                        GamepadAxisDir::Neg => r <= -0.5,
                    })
            }
        }
    }

    fn just_pressed(&self, input_state: &InputState<'_>) -> bool {
        match self {
            SingleKey::KeyCode(keycode) => input_state.input_keycode.just_pressed(*keycode),
            SingleKey::GamepadButton(button) => {
                input_state.input_gamepad_button.just_pressed(*button)
            }
            SingleKey::GamepadAxis(_axis, _dir) => {
                // TODO
                false
            }
        }
    }
}

macro_rules! any {
    ($x:expr, $($xs:expr),* $(,)?) => {
        [$($xs),*].into_iter().fold($x, |a, b| a.or(b))
    };
}
pub(crate) use any;

macro_rules! all {
    ($x:expr, $($xs:expr),* $(,)?) => {{
        [$($xs),*].into_iter().fold($x, |a, b| a.and(b))
    }};
}
pub(crate) use all;

macro_rules! keycode {
    ($code:ident) => {
        KeyAssign(vec![MultiKey(vec![SingleKey::KeyCode(KeyCode::$code)])])
    };
}
pub(crate) use keycode;

macro_rules! pad_button {
    ($id:literal, $button:ident) => {
        KeyAssign(vec![MultiKey(vec![SingleKey::GamepadButton(
            GamepadButton(Gamepad($id), GamepadButtonType::$button),
        )])])
    };
}
pub(crate) use pad_button;

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
