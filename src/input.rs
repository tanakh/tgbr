use bevy::prelude::*;
use enum_iterator::IntoEnumIterator;
use serde::{Deserialize, Serialize};
use tgbr_core::{Input as GameBoyInput, Pad};

use crate::config;

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

#[derive(Serialize, Deserialize)]
pub struct KeyConfig {
    pub up: KeyAssign,
    pub down: KeyAssign,
    pub left: KeyAssign,
    pub right: KeyAssign,
    pub a: KeyAssign,
    pub b: KeyAssign,
    pub start: KeyAssign,
    pub select: KeyAssign,
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

macro_rules! all {
    ($($assign:expr),* $(,)?) => {
        KeyAssign::All(vec![$($assign),*])
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

#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize, IntoEnumIterator)]
pub enum HotKey {
    Reset,
    Turbo,
    StateSave,
    StateLoad,
    NextSlot,
    PrevSlot,
    Rewind,
    Menu,
    FullScreen,
}

#[derive(Serialize, Deserialize)]
pub struct HotKeys(Vec<(HotKey, KeyAssign)>);

impl Default for HotKeys {
    fn default() -> Self {
        use HotKey::*;
        Self(vec![
            (Reset, all![keycode!(LControl), keycode!(R)]),
            (Turbo, any![keycode!(Tab), pad_button!(0, LeftTrigger)]),
            (StateSave, all![keycode!(LControl), keycode!(S)]),
            (StateLoad, all![keycode!(LControl), keycode!(L)]),
            (NextSlot, all![keycode!(LControl), keycode!(N)]),
            (PrevSlot, all![keycode!(LControl), keycode!(P)]),
            (
                Rewind,
                any![
                    keycode!(Back),
                    all![pad_button!(0, LeftTrigger2), pad_button!(0, RightTrigger2)]
                ],
            ),
            (Menu, keycode!(Escape)),
            (FullScreen, all![keycode!(RAlt), keycode!(Return)]),
        ])
    }
}

impl HotKeys {
    fn just_pressed(&self, hotkey: HotKey, input_state: &InputState<'_>) -> bool {
        self.0
            .iter()
            .find(|r| r.0 == hotkey)
            .map_or(false, |r| r.1.just_pressed(input_state))
    }
}

pub fn gameboy_input_system(
    config: Res<config::Config>,
    input_keycode: Res<Input<KeyCode>>,
    input_gamepad_button: Res<Input<GamepadButton>>,
    input_gamepad_axis: Res<Axis<GamepadAxis>>,
    mut input: ResMut<GameBoyInput>,
) {
    *input = config.key_config().input(&InputState::new(
        &input_keycode,
        &input_gamepad_button,
        &input_gamepad_axis,
    ));
}

pub fn check_hotkey(
    config: Res<config::Config>,
    input_keycode: Res<Input<KeyCode>>,
    input_gamepad_button: Res<Input<GamepadButton>>,
    input_gamepad_axis: Res<Axis<GamepadAxis>>,
    mut writer: EventWriter<HotKey>,
) {
    let input_state = InputState::new(&input_keycode, &input_gamepad_button, &input_gamepad_axis);

    for hotkey in HotKey::into_enum_iter() {
        if config.hotkeys().just_pressed(hotkey, &input_state) {
            writer.send(hotkey);
        }
    }
}
