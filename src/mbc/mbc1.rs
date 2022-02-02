use crate::{mbc::Mbc, rom::Rom, util::Ref};
use bitvec::prelude::*;

pub struct Mbc1 {
    rom: Ref<Rom>,
    ram: Vec<u8>,
    rom_bank: u8,
    ram_enable: bool,
    ram_bank: u8,
    rom_bank_mask: u8,
    ram_bank_mask: u8,
    banking_mode: bool,
}

impl Mbc1 {
    pub fn new(rom: &Ref<Rom>) -> Self {
        let rom_bank_mask = (rom.borrow().rom_size / 0x4000).saturating_sub(1) as u8;
        let ram_bank_mask = (rom.borrow().ram_size / 0x2000).saturating_sub(1) as u8;
        Self {
            rom: Ref::clone(rom),
            ram: vec![0; rom.borrow().ram_size],
            rom_bank: 1,
            ram_enable: false,
            ram_bank: 0,
            rom_bank_mask,
            ram_bank_mask,
            banking_mode: false,
        }
    }
}

impl Mbc for Mbc1 {
    fn read(&mut self, addr: u16) -> u8 {
        // TODO: Advanced ROM Banking Mode
        match addr {
            0x0000..=0x3FFF => self.rom.borrow().data[addr as usize],
            0x4000..=0x7FFF => {
                let offset = (self.rom_bank & self.rom_bank_mask) as usize * 0x4000;
                self.rom.borrow().data[offset + (addr & 0x3FFF) as usize]
            }
            0xA000..=0xBFFF => {
                let offset = (self.ram_bank & self.ram_bank_mask) as usize * 0x2000;
                self.ram[offset + (addr & 0x1FFF) as usize]
            }
            _ => unreachable!(),
        }
    }

    fn write(&mut self, addr: u16, data: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram_enable = data & 0x0F == 0x0A,
            0x2000..=0x3FFF => self.rom_bank.view_bits_mut::<Lsb0>()[0..=4]
                .store(data.view_bits::<Lsb0>()[0..=4].load::<u8>()),
            0x4000..=0x5FFF => self.ram_bank = data.view_bits::<Lsb0>()[0..=1].load(),
            0x6000..=0x7FFF => self.banking_mode = data & 0x01 != 0,
            0xA000..=0xBFFF => {
                // FIXME
                let addr = (addr & 0x1FFF) as usize;
                if addr < self.ram.len() {
                    self.ram[addr] = data;
                }
            }
            _ => unreachable!(),
        }
    }
}
