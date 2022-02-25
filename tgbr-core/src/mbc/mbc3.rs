use chrono::prelude::*;
use log::warn;
use serde::{Deserialize, Serialize};
use std::cmp::max;

use crate::{mbc::Context, rom::Rom, util::pack};

#[derive(Serialize, Deserialize)]
pub struct Mbc3 {
    rom_bank: u8,
    rom_bank_mask: u8,
    ram_bank_or_timer: RamBankOrTimer,
    ram_bank_mask: u8,
    ram_and_timer_enable: bool,
    clock_data_latch: DateTime<Utc>,
    clock_data_req: u8,
    day_counter_carry: bool,
}

#[derive(Serialize, Deserialize)]
enum RamBankOrTimer {
    RamBank(u8),
    Timer(u8),
    Invalid,
}

impl Mbc3 {
    pub fn new(rom: &Rom, internal_ram: Option<Vec<u8>>) -> Self {
        assert!(internal_ram.is_none());
        let rom_bank_num = rom.rom_size / 0x4000;
        assert!(rom_bank_num.is_power_of_two());
        let ram_bank_num = rom.ram_size / 0x2000;
        assert!(ram_bank_num.is_power_of_two());
        Self {
            rom_bank: 1,
            rom_bank_mask: rom_bank_num.saturating_sub(1) as u8,
            ram_bank_or_timer: RamBankOrTimer::Invalid,
            ram_bank_mask: ram_bank_num.saturating_sub(1) as u8,
            ram_and_timer_enable: false,
            clock_data_latch: Utc::now(),
            clock_data_req: !0,
            day_counter_carry: false,
        }
    }
}

impl super::MbcTrait for Mbc3 {
    fn read(&mut self, ctx: &mut impl Context, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3FFF => ctx.rom().data[addr as usize],
            0x4000..=0x7FFF => {
                let offset = (self.rom_bank & self.rom_bank_mask) as usize * 0x4000;
                ctx.rom().data[offset + (addr & 0x3FFF) as usize]
            }
            0xA000..=0xBFFF => {
                if self.ram_and_timer_enable {
                    match self.ram_bank_or_timer {
                        RamBankOrTimer::RamBank(ram_bank) => {
                            let offset = (ram_bank & self.ram_bank_mask) as usize * 0x2000;
                            ctx.external_ram()[offset + (addr & 0x1FFF) as usize]
                        }
                        RamBankOrTimer::Timer(ix) => match ix {
                            0x8 => self.clock_data_latch.second() as u8,
                            0x9 => self.clock_data_latch.minute() as u8,
                            0xA => self.clock_data_latch.hour() as u8,
                            0xB => (self.clock_data_latch.num_days_from_ce() & 0xff) as u8,
                            0xC => pack! {
                                0..=0 => (self.clock_data_latch.num_days_from_ce() >> 8) & 1,
                                1..=5 => !0,
                                6 => false,
                                7 => self.day_counter_carry,
                            },
                            _ => unreachable!(),
                        },
                        RamBankOrTimer::Invalid => !0,
                    }
                } else {
                    !0
                }
            }
            _ => unreachable!(),
        }
    }

    fn write(&mut self, ctx: &mut impl Context, addr: u16, data: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram_and_timer_enable = data & 0x0F == 0x0A,
            0x2000..=0x3FFF => self.rom_bank = max(1, data & 0x7F),
            0x4000..=0x5FFF => {
                if data <= 3 {
                    self.ram_bank_or_timer = RamBankOrTimer::RamBank(data);
                } else if 8 <= data && data <= 0xC {
                    self.ram_bank_or_timer = RamBankOrTimer::Timer(data);
                } else {
                    warn!("MBC3: invalid RAM bank or timer select: ${data:02}");
                    self.ram_bank_or_timer = RamBankOrTimer::Invalid;
                }
            }
            0x6000..=0x7FFF => {
                if !(data == 0 || data == 1) {
                    warn!("MBC3: Latch clock data: invalid data: ${data:02X}");
                }

                if self.clock_data_req == 0 && data == 1 {
                    let now = Utc::now();

                    if self.clock_data_latch.num_days_from_ce() & 0x1FF
                        > now.num_days_from_ce() & 0x1FF
                    {
                        self.day_counter_carry = true;
                    }

                    self.clock_data_latch = now;
                }
                self.clock_data_req = data;
            }
            0xA000..=0xBFFF => {
                if self.ram_and_timer_enable {
                    match self.ram_bank_or_timer {
                        RamBankOrTimer::RamBank(ram_bank) => {
                            let offset = (ram_bank & self.ram_bank_mask) as usize * 0x2000;
                            ctx.external_ram_mut()[offset + (addr & 0x1FFF) as usize] = data;
                        }
                        RamBankOrTimer::Timer(_) => {}
                        RamBankOrTimer::Invalid => {}
                    }
                }
            }
            _ => unreachable!(),
        }
    }
}
