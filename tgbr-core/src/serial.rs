use bitvec::prelude::*;
use log::trace;
use serde::{Deserialize, Serialize};

use crate::{
    consts::INT_SERIAL,
    context,
    interface::LinkCable,
    util::{pack, trait_alias},
};

#[derive(Serialize, Deserialize)]
pub struct SerialTransfer {
    buf: u8,
    recv_buf: Option<u8>,
    transfer_progress: bool,
    use_internal_clock: bool,
    transfer_timer: u64,
    transfer_pos: usize,
    #[serde(skip)]
    link_cable: Option<Box<dyn LinkCable + Send + Sync>>,
}

trait_alias!(pub trait Context = context::InterruptFlag);

impl SerialTransfer {
    pub fn new() -> Self {
        Self {
            buf: 0,
            recv_buf: None,
            transfer_progress: false,
            use_internal_clock: false,
            transfer_timer: 0,
            transfer_pos: 0,
            link_cable: None,
        }
    }

    pub fn set_link_cable(&mut self, link_cable: Option<Box<dyn LinkCable + Send + Sync>>) {
        self.link_cable = link_cable;
    }

    pub fn tick(&mut self, ctx: &mut impl Context) {
        if !self.transfer_progress {
            return;
        }

        // Check incomming data
        if self.recv_buf.is_none() {
            if let Some(r) = &mut self.link_cable {
                self.recv_buf = r.try_recv();
            }
        }

        let mut done = false;

        if self.use_internal_clock {
            // Transfer one bit per 128 tick (8192 Hz)
            self.transfer_timer += 1;
            if self.transfer_timer == 128 {
                self.transfer_timer = 0;
                self.transfer_pos += 1;

                if self.transfer_pos == 8 {
                    done = true;
                }
            }
        } else {
            // FIXME: wait when recieve data too fast
            if self.recv_buf.is_some() {
                done = true;
            }
        }

        if done {
            self.buf = self.recv_buf.unwrap_or(!0);
            self.recv_buf = None;
            self.transfer_pos = 0;
            self.transfer_progress = false;
            ctx.set_interrupt_flag_bit(INT_SERIAL);
        }
    }

    pub fn read_sb(&mut self) -> u8 {
        trace!("Read SB = ${:02X}", self.buf);
        self.buf
    }

    pub fn write_sb(&mut self, data: u8) {
        trace!("Write SB = ${data:02x}");
        self.buf = data;
    }

    pub fn read_sc(&mut self) -> u8 {
        let data = pack! {
            7     => self.transfer_progress,
            1..=6 => !0,
            0     => self.use_internal_clock,
        };
        trace!("Read SB = ${data:02X}");
        data
    }

    pub fn write_sc(&mut self, data: u8) {
        trace!("Write SC = ${data:02x}");
        let v = data.view_bits::<Lsb0>();
        self.use_internal_clock = v[0];
        if v[7] {
            if let Some(link_cable) = &mut self.link_cable {
                link_cable.send(self.buf);
            }
            self.transfer_progress = true;
            self.transfer_timer = 0;
            self.transfer_pos = 0;
            self.recv_buf = None;
        } else {
            self.transfer_progress = false;
        }
    }
}
