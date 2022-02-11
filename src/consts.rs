use std::ops::Range;

pub const CPU_CLOCK_PER_LINE: u64 = 114;
pub const DOTS_PER_LINE: u64 = CPU_CLOCK_PER_LINE * 4;
pub const LINES_PER_FRAME: u64 = 154;
pub const DOTS_PER_FRAME: u64 = DOTS_PER_LINE * LINES_PER_FRAME;
pub const VISIBLE_RANGE: Range<u64> = 0..144;

pub const SCREEN_WIDTH: u64 = 160;
pub const SCREEN_HEIGHT: u64 = 144;

pub const INT_VBLANK_BIT: usize = 0;
pub const INT_LCD_STAT_BIT: usize = 1;
pub const INT_TIMER_BIT: usize = 2;
pub const INT_SERIAL_BIT: usize = 3;
pub const INT_JOYPAD_BIT: usize = 4;

pub const INT_VBLANK: u8 = 1 << INT_VBLANK_BIT;
pub const INT_LCD_STAT: u8 = 1 << INT_LCD_STAT_BIT;
pub const INT_TIMER: u8 = 1 << INT_TIMER_BIT;
pub const INT_SERIAL: u8 = 1 << INT_SERIAL_BIT;
pub const INT_JOYPAD: u8 = 1 << INT_JOYPAD_BIT;

pub const AUDIO_SAMPLE_PER_FRAME: u64 = 800;
