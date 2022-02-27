use serde::{Deserialize, Serialize};

use crate::consts::{SCREEN_HEIGHT, SCREEN_WIDTH};

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Input {
    pub pad: Pad,
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Pad {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub a: bool,
    pub b: bool,
    pub start: bool,
    pub select: bool,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

#[derive(Clone)]
pub struct FrameBuffer {
    pub width: usize,
    pub height: usize,
    pub buf: Vec<Color>,
}

impl Default for FrameBuffer {
    fn default() -> Self {
        Self::new(SCREEN_WIDTH as _, SCREEN_HEIGHT as _)
    }
}

impl FrameBuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            buf: vec![Color::new(0, 0, 0); width * height],
        }
    }

    pub fn get(&self, x: usize, y: usize) -> Color {
        assert!(x < self.width);
        assert!(y < self.height);
        self.buf[y * self.width + x]
    }

    pub fn set(&mut self, x: usize, y: usize, color: Color) {
        assert!(x < self.width);
        assert!(y < self.height);
        self.buf[y * self.width + x] = color;
    }
}

#[derive(Default)]
pub struct AudioBuffer {
    pub buf: Vec<AudioSample>,
}

impl AudioBuffer {
    pub fn new() -> Self {
        Self { buf: vec![] }
    }
}

pub struct AudioSample {
    pub right: i16,
    pub left: i16,
}

impl AudioSample {
    pub fn new(right: i16, left: i16) -> Self {
        Self { right, left }
    }
}

pub trait LinkCable {
    fn send(&mut self, data: u8);
    fn try_recv(&mut self) -> Option<u8>;
}
