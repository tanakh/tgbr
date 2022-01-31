#[derive(Default, Clone, Debug)]
pub struct Input {
    pub pad: Pad,
}

#[derive(Default, Clone, Debug)]
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

#[derive(Clone, Copy)]
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

pub struct FrameBuffer {
    pub width: usize,
    pub height: usize,
    pub buf: Vec<Color>,
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
