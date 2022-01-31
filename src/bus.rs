use crate::{rom::Rom, util::Ref};

pub struct Bus {
    rom_bank: usize,
    vram: [u8; 0x2000],
    ram: [u8; 0x2000],
    oam: [u8; 0xA0],
    internal_ram: [u8; 0x7F],
    rom: Ref<Rom>,
}

impl Bus {
    pub fn new(rom: &Ref<Rom>) -> Self {
        Self {
            rom_bank: 0x4000,
            vram: [0; 0x2000],
            ram: [0; 0x2000],
            oam: [0; 0xA0],
            internal_ram: [0; 0x7F],
            rom: Ref::clone(rom),
        }
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3fff => self.rom.borrow().data[addr as usize],
            0x4000..=0x7fff => self.rom.borrow().data[(addr & 0x3fff) as usize + self.rom_bank],
            0x8000..=0x9fff => self.vram[(addr & 0x1fff) as usize],
            0xa000..=0xbfff => todo!("Switchable RAM bank"),
            0xc000..=0xdfff => self.ram[(addr & 0x1fff) as usize],
            0xe000..=0xfdff => self.ram[(addr & 0x1fff) as usize],
            0xfe00..=0xfe9f => self.oam[(addr & 0xff) as usize],
            0xfea0..=0xfeff => todo!("Unusable address: ${addr:04x}"),
            0xff00..=0xff7f => todo!("I/O"),
            0xff80..=0xfffe => self.internal_ram[(addr & 0x7f) as usize],
            0xffff => todo!("Interrupt Enable Register"),
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        todo!("Bus write: (${addr:04X}) = ${data:02X}")
    }

    pub fn read_immutable(&mut self, addr: u16) -> Option<u8> {
        match addr {
            0xff00..=0xff7f => None,
            _ => Some(self.read(addr)),
        }
    }
}
