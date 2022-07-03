use bitvec::prelude::*;
use serde::{Deserialize, Serialize};
use std::cmp::max;

use crate::{mbc::Context, rom::Rom};

#[derive(Serialize, Deserialize)]
pub struct Mbc1 {
    rom_bank: u8,
    high_bits: u8,
    rom_bank_mask: u8,
    ram_bank_mask: u8,
    ram_size_mask: u16,
    ram_enable: bool,
    banking_mode: bool,
}

impl Mbc1 {
    pub fn new(rom: &Rom, internal_ram: Option<Vec<u8>>) -> Self {
        assert!(internal_ram.is_none());
        assert!(!(rom.rom_size >= 1024 * 1024 && rom.ram_size >= 32 * 1024));

        let rom_bank_num = rom.rom_size / 0x4000;
        assert!(rom_bank_num.is_power_of_two());
        let ram_bank_num = rom.ram_size / 0x2000;
        assert!(rom.ram_size == 0 || ram_bank_num.is_power_of_two());
        Self {
            rom_bank: 1,
            high_bits: 0,
            rom_bank_mask: rom_bank_num.saturating_sub(1) as u8,
            ram_bank_mask: ram_bank_num.saturating_sub(1) as u8,
            ram_size_mask: rom.ram_size.saturating_sub(1) as u16,
            ram_enable: false,
            banking_mode: false,
        }
    }
}

impl super::MbcTrait for Mbc1 {
    fn read(&mut self, ctx: &mut impl Context, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3FFF => {
                let rom_bank = if !self.banking_mode {
                    0
                } else {
                    (self.high_bits << 5) & self.rom_bank_mask
                };
                ctx.rom().data[rom_bank as usize * 0x4000 + addr as usize]
            }
            0x4000..=0x7FFF => {
                let rom_bank = (self.high_bits << 5 | self.rom_bank) & self.rom_bank_mask;
                ctx.rom().data[rom_bank as usize * 0x4000 + (addr & 0x3FFF) as usize]
            }
            0xA000..=0xBFFF => {
                if self.ram_enable {
                    let ram_bank = if !self.banking_mode {
                        0
                    } else {
                        self.high_bits & self.ram_bank_mask
                    };
                    let addr = addr & 0x1FFF & self.ram_size_mask;
                    ctx.external_ram()[ram_bank as usize * 0x2000 + addr as usize]
                } else {
                    !0
                }
            }
            _ => unreachable!(),
        }
    }

    fn write(&mut self, ctx: &mut impl Context, addr: u16, data: u8) {
        match addr {
            0x0000..=0x1FFF => {
                log::debug!("MBC1: RAM enable: {data:02X}");
                self.ram_enable = data & 0x0F == 0x0A
            }
            0x2000..=0x3FFF => {
                log::debug!("MBC1: Bank low: {data:02X}");
                self.rom_bank.view_bits_mut::<Lsb0>()[0..=4].store(max(1, data & 0x1F));
            }
            0x4000..=0x5FFF => {
                log::debug!("MBC1: Bank high: {data:02X}");
                self.high_bits = data & 3;
            }
            0x6000..=0x7FFF => {
                log::debug!("MBC1: Banking mode: {data:02X}");
                self.banking_mode = data & 0x01 != 0;
            }
            0xA000..=0xBFFF => {
                if self.ram_enable {
                    let ram_bank = if !self.banking_mode {
                        0
                    } else {
                        self.high_bits & self.ram_bank_mask
                    };
                    let addr = addr & 0x1FFF & self.ram_size_mask;
                    ctx.external_ram_mut()[ram_bank as usize * 0x2000 + addr as usize] = data;
                }
            }
            _ => unreachable!(),
        }
    }
}
