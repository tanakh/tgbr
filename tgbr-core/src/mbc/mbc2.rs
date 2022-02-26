use bitvec::prelude::*;
use log::warn;
use serde::{Deserialize, Serialize};
use std::cmp::max;

#[derive(Serialize, Deserialize)]
pub struct Mbc2 {
    rom_bank: u8,
    rom_bank_mask: u8,
    #[serde(with = "serde_bytes")]
    ram: Vec<u8>,
    ram_enable: bool,
}

impl Mbc2 {
    pub fn new(rom: &crate::rom::Rom, internal_ram: Option<Vec<u8>>) -> Self {
        if let Some(ram) = &internal_ram {
            assert_eq!(ram.len(), 256);
        }
        let rom_bank_num = rom.rom_size / 0x4000;
        assert!(rom_bank_num.is_power_of_two());
        Self {
            rom_bank: 1,
            rom_bank_mask: rom_bank_num.saturating_sub(1) as u8,
            ram: internal_ram.unwrap_or_else(|| vec![0; 0x100]),
            ram_enable: false,
        }
    }
}

impl super::MbcTrait for Mbc2 {
    fn read(&mut self, ctx: &mut impl super::Context, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3FFF => ctx.rom().data[addr as usize],
            0x4000..=0x7FFF => {
                let offset = (self.rom_bank & self.rom_bank_mask) as usize * 0x4000;
                ctx.rom().data[offset + (addr & 0x3FFF) as usize]
            }
            0xA000..=0xBFFF => {
                if self.ram_enable {
                    let addr = (addr & 0x1FF) as usize;
                    let data = self.ram[addr / 2];
                    if addr % 2 == 0 {
                        data & 0xF
                    } else {
                        data >> 4
                    }
                } else {
                    !0
                }
            }
            _ => panic!("MBC2: Read ${addr:04X}"),
        }
    }
    fn write(&mut self, _ctx: &mut impl super::Context, addr: u16, data: u8) {
        match addr {
            0x0000..=0x3FFF => {
                if addr.view_bits::<Lsb0>()[8] {
                    self.rom_bank = max(1, data & 0xF);
                } else {
                    self.ram_enable = data == 0x0A;
                }
            }
            0xA000..=0xBFFF => {
                if self.ram_enable {
                    let addr = (addr & 0x1FF) as usize;
                    let v = self.ram[addr / 2].view_bits_mut::<Lsb0>();
                    if addr % 2 == 0 {
                        v[0..=3].store(data & 0xF);
                    } else {
                        v[4..=7].store(data & 0xF);
                    }
                }
            }
            _ => warn!("MBC2: Write ${addr:04X} = ${data:02X}"),
        }
    }
    fn internal_ram(&self) -> Option<&[u8]> {
        Some(&self.ram)
    }
}
