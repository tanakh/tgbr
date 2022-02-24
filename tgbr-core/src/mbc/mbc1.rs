use bitvec::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{mbc::Context, rom::Rom};

#[derive(Serialize, Deserialize)]
pub struct Mbc1 {
    rom_bank: u8,
    rom_bank_mask: u8,
    ram_bank: u8,
    ram_bank_mask: u8,
    ram_enable: bool,
    banking_mode: bool,
}

impl Mbc1 {
    pub fn new(rom: &Rom, internal_ram: Option<Vec<u8>>) -> Self {
        assert!(internal_ram.is_none());
        let rom_bank_num = rom.rom_size / 0x4000;
        assert!(rom_bank_num.is_power_of_two());
        let ram_bank_num = rom.rom_size / 0x2000;
        assert!(ram_bank_num.is_power_of_two());
        Self {
            rom_bank: 1,
            rom_bank_mask: rom_bank_num.saturating_sub(1) as u8,
            ram_bank: 0,
            ram_bank_mask: ram_bank_num.saturating_sub(1) as u8,
            ram_enable: false,
            banking_mode: false,
        }
    }
}

impl super::MbcTrait for Mbc1 {
    fn read(&mut self, ctx: &mut impl Context, addr: u16) -> u8 {
        // TODO: Advanced ROM Banking Mode
        match addr {
            0x0000..=0x3FFF => ctx.rom().data[addr as usize],
            0x4000..=0x7FFF => {
                let offset = (self.rom_bank & self.rom_bank_mask) as usize * 0x4000;
                ctx.rom().data[offset + (addr & 0x3FFF) as usize]
            }
            0xA000..=0xBFFF => {
                if self.ram_enable {
                    let offset = (self.ram_bank & self.ram_bank_mask) as usize * 0x2000;
                    ctx.external_ram()[offset + (addr & 0x1FFF) as usize]
                } else {
                    !0
                }
            }
            _ => unreachable!(),
        }
    }

    fn write(&mut self, ctx: &mut impl Context, addr: u16, data: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram_enable = data & 0x0F == 0x0A,
            0x2000..=0x3FFF => self.rom_bank.view_bits_mut::<Lsb0>()[0..=4]
                .store(data.view_bits::<Lsb0>()[0..=4].load::<u8>()),
            0x4000..=0x5FFF => self.ram_bank = data & 3,
            0x6000..=0x7FFF => self.banking_mode = data & 0x01 != 0,
            0xA000..=0xBFFF => {
                if self.ram_enable {
                    let offset = (self.ram_bank & self.ram_bank_mask) as usize * 0x2000;
                    ctx.external_ram_mut()[offset + (addr & 0x1FFF) as usize] = data;
                }
            }
            _ => unreachable!(),
        }
    }
}
