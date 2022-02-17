use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use tgbr_core::{Input as GameBoyInput, Pad};

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
}

#[derive(Serialize, Deserialize)]
pub struct KeyConfig {
    up: KeyAssign,
    down: KeyAssign,
    left: KeyAssign,
    right: KeyAssign,
    a: KeyAssign,
    b: KeyAssign,
    start: KeyAssign,
    select: KeyAssign,
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

struct InputState<'a> {
    input_keycode: &'a Input<KeyCode>,
    input_gamepad_button: &'a Input<GamepadButton>,
    input_gamepad_axis: &'a Axis<GamepadAxis>,
}

#[derive(PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum HotKey {
    Reset,
    Turbo,
    StateSave,
    StateLoad,
    NextSlot,
    PrevSlot,
    Rewind,
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
            (FullScreen, all![keycode!(RAlt), keycode!(Return)]),
        ])
    }
}

pub fn gameboy_input_system(
    key_config: Res<KeyConfig>,
    input_keycode: Res<Input<KeyCode>>,
    input_gamepad_button: Res<Input<GamepadButton>>,
    input_gamepad_axis: Res<Axis<GamepadAxis>>,
    mut input: ResMut<GameBoyInput>,
) {
    *input = key_config.input(&InputState {
        input_keycode: &input_keycode,
        input_gamepad_button: &input_gamepad_button,
        input_gamepad_axis: &input_gamepad_axis,
    });
}

// macro_rules! pad_axis {
//     ($id:expr, $axis:ident) => {
//         KeyAssign::PadAxis {
//             id: $id,
//             axis: AssignPadAxis,
//         }
//     };
// }

// #[derive(PartialEq, Eq, Clone, Copy)]
// pub enum PadButton {
//     Up,
//     Down,
//     Left,
//     Right,
//     A,
//     B,
//     Start,
//     Select,
// }

// pub struct KeyConfig(Vec<(PadButton, KeyAssign)>);

// impl Default for KeyConfig {
//     fn default() -> Self {
//         use PadButton::*;
//         Self(vec![
//             (Up, any![kbd!(Up), pad_button!(0, DPadUp)]),
//             (Down, any![kbd!(Down), pad_button!(0, DPadDown)]),
//             (Left, any![kbd!(Left), pad_button!(0, DPadLeft)]),
//             (Right, any![kbd!(Right), pad_button!(0, DPadRight)]),
//             (A, any![kbd!(Z), pad_button!(0, A)]),
//             (B, any![kbd!(X), pad_button!(0, X)]),
//             (Start, any![kbd!(Return), pad_button!(0, Start)]),
//             (Select, any![kbd!(RShift), pad_button!(0, Back)]),
//         ])
//     }
// }

// #[derive(PartialEq, Eq, Clone)]
// enum Key {
//     PadButton(PadButton),
//     HotKey(HotKey),
// }

// pub struct KeyState {
//     key: Key,
//     key_assign: KeyAssign,
//     pressed: bool,
//     prev_pressed: bool,
// }

// impl KeyState {
//     pub fn pressed(&self) -> bool {
//         self.pressed
//     }

//     pub fn pushed(&self) -> bool {
//         self.pressed && !self.prev_pressed
//     }

//     pub fn update(&mut self, pressed: bool) {
//         self.prev_pressed = self.pressed;
//         self.pressed = pressed;
//     }
// }

// static NULL_KEY: KeyState = KeyState {
//     key: Key::PadButton(PadButton::Up),
//     key_assign: any![],
//     pressed: false,
//     prev_pressed: false,
// };

// impl InputManager {
//     pub fn new(key_config: &KeyConfig, hotkeys: &HotKeys) -> Result<Self> {
//         // let gcs = sdl.game_controller().map_err(|e| anyhow!("{e}"))?;

//         // let controllers = (0..(gcs.num_joysticks().map_err(|e| anyhow!("{e}"))?))
//         //     .map(|id| gcs.open(id))
//         //     .collect::<Result<Vec<_>, _>>()?;

//         let mut key_states = vec![];

//         for r in &key_config.0 {
//             key_states.push(KeyState {
//                 key: Key::PadButton(r.0.clone()),
//                 key_assign: r.1.clone(),
//                 pressed: false,
//                 prev_pressed: false,
//             });
//         }

//         for r in &hotkeys.0 {
//             key_states.push(KeyState {
//                 key: Key::HotKey(r.0.clone()),
//                 key_assign: r.1.clone(),
//                 pressed: false,
//                 prev_pressed: false,
//             });
//         }

//         Ok(Self {
//             // controllers,
//             // key_states,
//         })
//     }

//     // pub fn update(&mut self, e: &EventPump) {
//     //     let kbstate = keyboard::KeyboardState::new(e);

//     //     // for i in 0..self.key_states.len() {}
//     //     for r in &mut self.key_states {
//     //         let pressed = check_pressed(&kbstate, &self.controllers, &r.key_assign);
//     //         r.update(pressed);
//     //     }
//     // }

//     pub fn pad_button(&self, pad_button: PadButton) -> &KeyState {
//         // self.key_states
//         //     .iter()
//         //     .find(|r| &r.key == &Key::PadButton(pad_button))
//         //     .unwrap_or(&NULL_KEY)
//         todo!()
//     }

//     pub fn hotkey(&self, hotkey: HotKey) -> &KeyState {
//         // self.key_states
//         //     .iter()
//         //     .find(|r| &r.key == &Key::HotKey(hotkey))
//         //     .unwrap_or(&NULL_KEY)
//         todo!()
//     }
// }
