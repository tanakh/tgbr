use std::ops::Range;

pub const CPU_CLOCK_PER_LINE: u64 = 114;
pub const DOTS_PER_LINE: u64 = CPU_CLOCK_PER_LINE * 4;
pub const LINES_PER_FRAME: u64 = 154;
pub const DOTS_PER_FRAME: u64 = DOTS_PER_LINE * LINES_PER_FRAME;
pub const VISIBLE_RANGE: Range<u64> = 0..144;

pub const SCREEN_WIDTH: u64 = 160;
pub const SCREEN_HEIGHT: u64 = 144;

pub const INT_VBLANK: u8 = 1 << 0;
pub const INT_LCD_STAT: u8 = 1 << 1;
pub const INT_TIMER: u8 = 1 << 2;
pub const INT_SERIAL: u8 = 1 << 3;
pub const INT_JOYPAD: u8 = 1 << 4;

pub const AUDIO_SAMPLE_PER_FRAME: u64 = 800;
