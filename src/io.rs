use bitvec::prelude::*;
use log::{info, trace, warn};

use crate::{
    apu::Apu,
    consts::{INT_JOYPAD, INT_TIMER},
    interface::Input,
    ppu::Ppu,
    util::Ref,
};

pub struct Io {
    select_action_buttons: bool,
    select_direction_buttons: bool,
    divider: u8,
    timer_counter: u8,
    timer_modulo: u8,
    timer_enable: bool,
    input_clock_select: u8,
    interrupt_flag: Ref<u8>,
    interrupt_enable: Ref<u8>,

    divider_counter: u8,
    timer_divider_counter: u16,

    ppu: Ref<Ppu>,
    apu: Ref<Apu>,

    input: Input,
}

const TIMER_DIVIDER_BITS: [u8; 4] = [8, 2, 4, 6];

impl Io {
    pub fn new(
        ppu: &Ref<Ppu>,
        apu: &Ref<Apu>,
        interrupt_enable: &Ref<u8>,
        interrupt_flag: &Ref<u8>,
    ) -> Self {
        Self {
            select_action_buttons: false,
            select_direction_buttons: false,
            divider: 0,
            timer_counter: 0,
            timer_modulo: 0,
            timer_enable: false,
            input_clock_select: 0,
            interrupt_enable: Ref::clone(interrupt_enable),
            interrupt_flag: Ref::clone(interrupt_flag),
            divider_counter: 0,
            timer_divider_counter: 0,
            ppu: Ref::clone(ppu),
            apu: Ref::clone(apu),
            input: Input::default(),
        }
    }

    pub fn tick(&mut self) {
        for _ in 0..4 {
            self.ppu.borrow_mut().tick();
            self.apu.borrow_mut().tick();
        }

        self.divider_counter += 1;
        if self.divider_counter == 64 {
            self.divider_counter = 0;
            self.divider = self.divider.wrapping_add(1);
        }

        if self.timer_enable {
            let prev = self.timer_divider_counter;
            let new = prev.wrapping_add(1);
            self.timer_divider_counter = new;
            let pos = TIMER_DIVIDER_BITS[self.input_clock_select as usize] as usize;

            if (prev & !new).view_bits::<Lsb0>()[pos - 1] {
                // eprintln!(
                //     "TIMER TICK: {:04X}, clock-select: {}, timer-counter: {:02X}",
                //     self.timer_divider_counter, self.input_clock_select, self.timer_counter
                // );

                let (new_counter, overflow) = self.timer_counter.overflowing_add(1);
                self.timer_counter = new_counter;
                if overflow {
                    self.timer_counter = self.timer_modulo;
                    *self.interrupt_flag.borrow_mut() |= INT_TIMER;
                }
            }
        }
    }

    pub fn set_input(&mut self, input: &Input) {
        let prev_lines = self.keypad_input_lines();
        self.input = input.clone();
        let cur_lines = self.keypad_input_lines();

        for i in 0..4 {
            if prev_lines[i] && !cur_lines[i] {
                *self.interrupt_flag.borrow_mut() |= INT_JOYPAD;
            }
        }
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

    pub fn read(&mut self, addr: u16) -> u8 {
        let ret = match addr & 0xff {
            // P1: Joypad (R/W)
            0x00 => {
                let lines = self.keypad_input_lines();
                let mut ret = 0;
                let v = ret.view_bits_mut::<Lsb0>();
                v.set(5, self.select_action_buttons);
                v.set(4, self.select_direction_buttons);
                v.set(3, lines[3]);
                v.set(2, lines[2]);
                v.set(1, lines[1]);
                v.set(0, lines[0]);
                ret
            }
            // SB: Serial transfer data (R/W)
            0x01 => {
                warn!("Read from SB");
                0x00
            }
            // SC: Serial transfer control (R/W)
            0x02 => {
                warn!("Read from SC");
                0x00
            }
            // DIV: Divider register (R/W)
            0x04 => self.divider,
            // TIMA: Timer counter (R/W)
            0x05 => self.timer_counter,
            // TMA: Timer modulo (R/W)
            0x06 => self.timer_modulo,
            // TAC: Timer control (R/W)
            0x07 => {
                let mut ret = 0;
                let v = ret.view_bits_mut::<Lsb0>();
                v.set(2, self.timer_enable);
                v[0..=1].store(self.input_clock_select);
                ret
            }
            // IF: Interrupt flag (R/W)
            0x0f => *self.interrupt_flag.borrow(),
            // IE: Interrupt enable (R/W)
            0xff => *self.interrupt_enable.borrow(),

            // APU Registers
            0x10..=0x3F => self.apu.borrow_mut().read(addr),
            // PPU Registers
            0x40..=0x4B => self.ppu.borrow_mut().read(addr),

            _ => {
                warn!("Unknown I/O Read: {:04X}", addr);
                0
            }
        };

        trace!("I/O Read: (${addr:04X}) => ${ret:02X}");
        ret
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        trace!("I/O write: ${addr:04X} = ${data:02X}");

        match addr & 0xff {
            // P1: Joypad (R/W)
            0x00 => {
                let v = data.view_bits::<Lsb0>();
                self.select_direction_buttons = v[4];
                self.select_action_buttons = v[5];
            }
            // SB: Serial transfer data (R/W)
            0x01 => {
                info!("Write to SB: ${data:02X} = {}", data as char);
            }
            // SC: Serial transfer control (R/W)
            0x02 => {
                info!("Write to SC: {data:02X}");
            }
            // DIV: Divider register (R/W)
            0x04 => self.divider = 0,
            // TIMA: Timer counter (R/W)
            0x05 => self.timer_counter = data,
            // TMA: Timer modulo (R/W)
            0x06 => self.timer_modulo = data,
            // TAC: Timer control (R/W)
            0x07 => {
                let v = data.view_bits::<Lsb0>();
                self.timer_enable = v[2];
                self.input_clock_select = v[0..=1].load();
            }
            // IF: Interrupt flag (R/W)
            0x0f => *self.interrupt_flag.borrow_mut() = data & 0x1f,
            // IE: Interrupt enable (R/W)
            0xff => *self.interrupt_enable.borrow_mut() = data & 0x1f,

            // APU Registers
            0x10..=0x3F => self.apu.borrow_mut().write(addr, data),
            // PPU Registers
            0x40..=0x4B => self.ppu.borrow_mut().write(addr, data),

            _ => {
                warn!("Write to ${:04X} = ${:02X}", addr, data);
            }
        }
    }
}
