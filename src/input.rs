use anyhow::{anyhow, Result};
use sdl2::{
    controller::{self, GameController},
    keyboard, EventPump, Sdl,
};
use tgbr_core::interface::{Input, Pad};

#[derive(Clone)]
enum KeyAssign {
    Keyboard {
        scancode: keyboard::Scancode,
    },
    PadButton {
        id: usize,
        button: controller::Button,
    },
    PadAxis {
        id: usize,
        axis: controller::Axis,
    },
    All(Vec<KeyAssign>),
    Any(Vec<KeyAssign>),
}

macro_rules! kbd {
    ($scancode:ident) => {
        KeyAssign::Keyboard {
            scancode: sdl2::keyboard::Scancode::$scancode,
        }
    };
}

macro_rules! pad_button {
    ($id:expr, $button:ident) => {
        KeyAssign::PadButton {
            id: $id,
            button: controller::Button::$button,
        }
    };
}

macro_rules! pad_axis {
    ($id:expr, $axis:ident) => {
        KeyAssign::PadAxis {
            id: $id,
            axis: controller::Axis::$axis,
        }
    };
}

macro_rules! any {
    ($($key:expr),* $(,)?) => {
        KeyAssign::Any(vec![$($key),*])
    };
}

macro_rules! all {
    ($($key:expr),* $(,)?) => {
        KeyAssign::All(vec![$($key),*])
    };
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum PadButton {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    Start,
    Select,
}

pub struct KeyConfig(Vec<(PadButton, KeyAssign)>);

impl Default for KeyConfig {
    fn default() -> Self {
        use PadButton::*;
        Self(vec![
            (Up, any![kbd!(Up), pad_button!(0, DPadUp)]),
            (Down, any![kbd!(Down), pad_button!(0, DPadDown)]),
            (Left, any![kbd!(Left), pad_button!(0, DPadLeft)]),
            (Right, any![kbd!(Right), pad_button!(0, DPadRight)]),
            (A, any![kbd!(Z), pad_button!(0, A)]),
            (B, any![kbd!(X), pad_button!(0, X)]),
            (Start, any![kbd!(Return), pad_button!(0, Start)]),
            (Select, any![kbd!(RShift), pad_button!(0, Back)]),
        ])
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum HotKey {
    Reset,
    Turbo,
    StateSave,
    StateLoad,
    NextSlot,
    PrevSlot,
    FullScreen,
}

pub struct HotKeys(Vec<(HotKey, KeyAssign)>);

impl Default for HotKeys {
    fn default() -> Self {
        use HotKey::*;
        Self(vec![
            (Reset, all![kbd!(LCtrl), kbd!(R)]),
            (Turbo, any![kbd!(Tab), pad_axis!(0, TriggerLeft)]),
            (StateSave, all![kbd!(LCtrl), kbd!(S)]),
            (StateLoad, all![kbd!(LCtrl), kbd!(L)]),
            (NextSlot, all![kbd!(LCtrl), kbd!(N)]),
            (PrevSlot, all![kbd!(LCtrl), kbd!(P)]),
            (FullScreen, all![kbd!(RAlt), kbd!(Return)]),
        ])
    }
}

#[derive(PartialEq, Eq, Clone)]
enum Key {
    PadButton(PadButton),
    HotKey(HotKey),
}

pub struct KeyState {
    key: Key,
    key_assign: KeyAssign,
    pressed: bool,
    prev_pressed: bool,
}

impl KeyState {
    pub fn pressed(&self) -> bool {
        self.pressed
    }

    pub fn pushed(&self) -> bool {
        self.pressed && !self.prev_pressed
    }

    pub fn update(&mut self, pressed: bool) {
        self.prev_pressed = self.pressed;
        self.pressed = pressed;
    }
}

pub struct InputManager {
    controllers: Vec<GameController>,
    key_states: Vec<KeyState>,
}

static NULL_KEY: KeyState = KeyState {
    key: Key::PadButton(PadButton::Up),
    key_assign: any![],
    pressed: false,
    prev_pressed: false,
};

impl InputManager {
    pub fn new(sdl: &Sdl, key_config: &KeyConfig, hotkeys: &HotKeys) -> Result<Self> {
        let gcs = sdl.game_controller().map_err(|e| anyhow!("{e}"))?;

        let controllers = (0..(gcs.num_joysticks().map_err(|e| anyhow!("{e}"))?))
            .map(|id| gcs.open(id))
            .collect::<Result<Vec<_>, _>>()?;

        let mut key_states = vec![];

        for r in &key_config.0 {
            key_states.push(KeyState {
                key: Key::PadButton(r.0.clone()),
                key_assign: r.1.clone(),
                pressed: false,
                prev_pressed: false,
            });
        }

        for r in &hotkeys.0 {
            key_states.push(KeyState {
                key: Key::HotKey(r.0.clone()),
                key_assign: r.1.clone(),
                pressed: false,
                prev_pressed: false,
            });
        }

        Ok(Self {
            controllers,
            key_states,
        })
    }

    pub fn update(&mut self, e: &EventPump) {
        let kbstate = keyboard::KeyboardState::new(e);

        // for i in 0..self.key_states.len() {}
        for r in &mut self.key_states {
            let pressed = check_pressed(&kbstate, &self.controllers, &r.key_assign);
            r.update(pressed);
        }
    }

    pub fn input(&self) -> Input {
        use PadButton::*;
        Input {
            pad: Pad {
                up: self.pad_button(Up).pressed(),
                down: self.pad_button(Down).pressed(),
                left: self.pad_button(Left).pressed(),
                right: self.pad_button(Right).pressed(),
                a: self.pad_button(A).pressed(),
                b: self.pad_button(B).pressed(),
                start: self.pad_button(Start).pressed(),
                select: self.pad_button(Select).pressed(),
            },
        }
    }

    pub fn pad_button(&self, pad_button: PadButton) -> &KeyState {
        self.key_states
            .iter()
            .find(|r| &r.key == &Key::PadButton(pad_button))
            .unwrap_or(&NULL_KEY)
    }

    pub fn hotkey(&self, hotkey: HotKey) -> &KeyState {
        self.key_states
            .iter()
            .find(|r| &r.key == &Key::HotKey(hotkey))
            .unwrap_or(&NULL_KEY)
    }
}

fn check_pressed(
    kbstate: &keyboard::KeyboardState<'_>,
    controllers: &[GameController],
    key: &KeyAssign,
) -> bool {
    use KeyAssign::*;
    match key {
        Keyboard { scancode } => kbstate.is_scancode_pressed(*scancode),
        PadButton { id, button } => controllers.get(*id).map_or(false, |r| r.button(*button)),
        PadAxis { id, axis } => controllers
            .get(*id)
            .map_or(false, |r| dbg!(r.axis(*axis)) > 32767 / 2),
        All(keys) => keys.iter().all(|k| check_pressed(kbstate, controllers, k)),
        Any(keys) => keys.iter().any(|k| check_pressed(kbstate, controllers, k)),
    }
}
