use bitvec::prelude::*;
use log::{trace, warn};
use serde::{Deserialize, Serialize};

use crate::{
    consts::{INT_JOYPAD, INT_TIMER},
    context,
    interface::{Input, LinkCable},
    serial::SerialTransfer,
    util::{pack, trait_alias},
};

trait_alias!(pub trait Context = context::InterruptFlag + context::Ppu + context::Apu);

#[derive(Serialize, Deserialize)]
pub struct Io {
    select_action_buttons: bool,
    select_direction_buttons: bool,

    divider: u16,
    timer_counter: u8,
    timer_modulo: u8,
    timer_enable: bool,
    input_clock_select: u8,
    prev_timer_clock: bool,
    timer_reload: bool,
    timer_reloaded: bool,

    serial: SerialTransfer,
    input: Input,
}

impl Io {
    pub fn new() -> Self {
        Self {
            select_action_buttons: false,
            select_direction_buttons: false,
            divider: 0,
            timer_counter: 0,
            timer_modulo: 0,
            timer_enable: false,
            input_clock_select: 0,
            prev_timer_clock: false,
            timer_reload: false,
            timer_reloaded: false,
            serial: SerialTransfer::new(),
            input: Input::default(),
        }
    }

    pub fn tick(&mut self, ctx: &mut impl Context) {
        self.divider = self.divider.wrapping_add(4);

        self.timer_reloaded = false;

        if self.timer_reload {
            log::trace!("Timer reload: ${:02X}", self.timer_modulo);
            self.timer_counter = self.timer_modulo;
            ctx.set_interrupt_flag_bit(INT_TIMER);
            self.timer_reload = false;
            self.timer_reloaded = true;
        }

        const TIMER_DIVIDER_BITS: [u8; 4] = [9, 3, 5, 7];
        let clock_bit = TIMER_DIVIDER_BITS[self.input_clock_select as usize] as usize;
        let timer_clock = self.timer_enable && self.divider.view_bits::<Lsb0>()[clock_bit];

        // Counting on falling edge
        if self.prev_timer_clock && !timer_clock {
            let (new_counter, overflow) = self.timer_counter.overflowing_add(1);
            self.timer_counter = new_counter;
            if overflow {
                log::trace!("Timer overflow");
                self.timer_reload = true;
            }
        }

        self.prev_timer_clock = timer_clock;
    }

    pub fn set_input(&mut self, ctx: &mut impl Context, input: &Input) {
        let prev_lines = self.keypad_input_lines();
        self.input = input.clone();
        let cur_lines = self.keypad_input_lines();

        for i in 0..4 {
            if prev_lines[i] && !cur_lines[i] {
                ctx.set_interrupt_flag_bit(INT_JOYPAD);
            }
        }
    }

    pub fn serial(&mut self) -> &mut SerialTransfer {
        &mut self.serial
    }

    pub fn set_link_cable(&mut self, link_cable: Option<Box<dyn LinkCable + Send + Sync>>) {
        self.serial.set_link_cable(link_cable);
    }

    fn keypad_input_lines(&self) -> [bool; 4] {
        let mut lines = [true; 4];
        let r = &self.input.pad;
        if !self.select_action_buttons {
            lines[0] &= !r.a;
            lines[1] &= !r.b;
            lines[2] &= !r.select;
            lines[3] &= !r.start;
        }
        if !self.select_direction_buttons {
            lines[0] &= !r.right;
            lines[1] &= !r.left;
            lines[2] &= !r.up;
            lines[3] &= !r.down;
        }
        lines
    }

    pub fn read(&mut self, ctx: &mut impl Context, addr: u16) -> u8 {
        let ret = match addr & 0xff {
            // P1: Joypad (R/W)
            0x00 => {
                let lines = self.keypad_input_lines();
                pack! {
                    6..=7 => !0,
                    5 => self.select_action_buttons,
                    4 => self.select_direction_buttons,
                    3 => lines[3],
                    2 => lines[2],
                    1 => lines[1],
                    0 => lines[0],
                }
            }
            // SB: Serial transfer data (R/W)
            0x01 => self.serial.read_sb(),
            // SC: Serial transfer control (R/W)
            0x02 => self.serial.read_sc(),
            // DIV: Divider register (R/W)
            0x04 => (self.divider >> 8) as u8,
            // TIMA: Timer counter (R/W)
            0x05 => self.timer_counter,
            // TMA: Timer modulo (R/W)
            0x06 => self.timer_modulo,
            // TAC: Timer control (R/W)
            0x07 => pack! {
                3..=7 => !0,
                2     => self.timer_enable,
                0..=1 => self.input_clock_select,
            },
            // IF: Interrupt flag (R/W)
            0x0f => pack! {
                5..=7 => !0,
                0..=4 => ctx.interrupt_flag(),
            },
            // IE: Interrupt enable (R/W)
            0xff => pack! {
                0..=7 => ctx.interrupt_enable(),
            },

            // APU Registers
            0x10..=0x3F => ctx.apu_mut().read(addr),
            // PPU Registers
            0x40..=0x4B => ctx.ppu_mut().read(addr),

            _ => {
                warn!("Unknown I/O Read: {:04X}", addr);
                !0
            }
        };

        trace!("I/O Read: (${addr:04X}) => ${ret:02X}");
        ret
    }

    pub fn write(&mut self, ctx: &mut impl Context, addr: u16, data: u8) {
        trace!("I/O write: ${addr:04X} = ${data:02X}");

        match addr & 0xff {
            // P1: Joypad (R/W)
            0x00 => {
                let v = data.view_bits::<Lsb0>();
                self.select_direction_buttons = v[4];
                self.select_action_buttons = v[5];
            }
            // SB: Serial transfer data (R/W)
            0x01 => self.serial.write_sb(data),
            // SC: Serial transfer control (R/W)
            0x02 => self.serial.write_sc(data),
            // DIV: Divider register (R/W)
            0x04 => self.divider = 0,
            // TIMA: Timer counter (R/W)
            0x05 => {
                // On the reload delay cycle, cancel reloading
                if self.timer_reload {
                    self.timer_reload = false;
                }
                // On the timer reloaded cycle, ignore writing to TIMA
                if !self.timer_reloaded {
                    self.timer_counter = data;
                }
            }
            // TMA: Timer modulo (R/W)
            0x06 => {
                self.timer_modulo = data;
                // On the timer reloaded cycle, this value is loaded into TIMA
                if self.timer_reloaded {
                    self.timer_counter = data;
                }
            }
            // TAC: Timer control (R/W)
            0x07 => {
                let v = data.view_bits::<Lsb0>();
                self.timer_enable = v[2];
                self.input_clock_select = v[0..=1].load();
            }
            // IF: Interrupt flag (R/W)
            0x0f => ctx.set_interrupt_flag(data & 0x1f),
            // IE: Interrupt enable (R/W)
            0xff => {
                trace!("IE = {data:02X}");
                ctx.set_interrupt_enable(data)
            }

            // APU Registers
            0x10..=0x3F => ctx.apu_mut().write(addr, data),
            // PPU Registers
            0x40..=0x4B => ctx.ppu_mut().write(addr, data),

            _ => {
                warn!("Write to ${:04X} = ${:02X}", addr, data);
            }
        }
    }
}
