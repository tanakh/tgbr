use log::{trace, warn};

use crate::{io::Io, mbc::Mbc, util::Ref};

pub struct Bus {
    ram: [u8; 0x2000],
    vram: Ref<Vec<u8>>,
    oam: Ref<Vec<u8>>,
    hiram: [u8; 0x7F],
    mbc: Ref<dyn Mbc>,
    io: Ref<Io>,
}

impl Bus {
    pub fn new(mbc: &Ref<dyn Mbc>, vram: &Ref<Vec<u8>>, oam: &Ref<Vec<u8>>, io: &Ref<Io>) -> Self {
        Self {
            ram: [0; 0x2000],
            vram: Ref::clone(vram),
            oam: Ref::clone(oam),
            hiram: [0; 0x7F],
            mbc: Ref::clone(mbc),
            io: Ref::clone(io),
        }
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        let data = match addr {
            0x0000..=0x7fff => self.mbc.borrow_mut().read(addr),
            0x8000..=0x9fff => self.vram.borrow()[(addr & 0x1fff) as usize],
            0xa000..=0xbfff => self.mbc.borrow_mut().read(addr),
            0xc000..=0xfdff => self.ram[(addr & 0x1fff) as usize],
            0xfe00..=0xfe9f => self.oam.borrow()[(addr & 0xff) as usize],
            0xfea0..=0xfeff => todo!("Read from Unusable address: ${addr:04x}"),
            0xff00..=0xff7f => self.io.borrow_mut().read(addr),
            0xff80..=0xfffe => self.hiram[(addr & 0x7f) as usize],
            0xffff => self.io.borrow_mut().read(addr),
        };
        trace!("Read: ${addr:04X} = ${data:02X}");
        data
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        trace!("Write: ${addr:04X} = ${data:02X}");
        match addr {
            0x0000..=0x7fff => self.mbc.borrow_mut().write(addr, data),
            0x8000..=0x9fff => self.vram.borrow_mut()[(addr & 0x1fff) as usize] = data,
            0xa000..=0xbfff => self.mbc.borrow_mut().write(addr, data),
            0xc000..=0xfdff => self.ram[(addr & 0x1fff) as usize] = data,
            0xfe00..=0xfe9f => self.oam.borrow_mut()[(addr & 0xff) as usize] = data,
            0xfea0..=0xfeff => warn!("Write to Unusable address: ${addr:04X} = ${data:02X}"),
            0xff00..=0xff7f => self.io.borrow_mut().write(addr, data),
            0xff80..=0xfffe => self.hiram[(addr & 0x7f) as usize] = data,
            0xffff => self.io.borrow_mut().write(addr, data),
        };
    }

    pub fn read_immutable(&mut self, addr: u16) -> Option<u8> {
        let data = match addr {
            0x0000..=0x7fff => self.mbc.borrow_mut().read(addr),
            0x8000..=0x9fff => self.vram.borrow()[(addr & 0x1fff) as usize],
            0xa000..=0xbfff => self.mbc.borrow_mut().read(addr),
            0xc000..=0xfdff => self.ram[(addr & 0x1fff) as usize],
            0xfe00..=0xfe9f => self.oam.borrow()[(addr & 0xff) as usize],
            0xfea0..=0xfeff => todo!("Read from Unusable address: ${addr:04x}"),
            0xff00..=0xff7f => return None,
            0xff80..=0xfffe => self.hiram[(addr & 0x7f) as usize],
            0xffff => return None,
        };
        Some(data)
    }

    pub fn tick(&mut self) {
        self.io.borrow_mut().tick();
    }
}
