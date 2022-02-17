#![recursion_limit = "1024"]

pub mod apu;
pub mod bus;
pub mod config;
pub mod consts;
pub mod context;
pub mod cpu;
pub mod gameboy;
pub mod interface;
pub mod io;
pub mod mbc;
pub mod ppu;
pub mod rom;
pub mod serial;
pub mod util;

pub use crate::{
    config::{Config, Model},
    gameboy::GameBoy,
    interface::{AudioBuffer, Color, FrameBuffer, Input, LinkCable, Pad},
    rom::Rom,
};
