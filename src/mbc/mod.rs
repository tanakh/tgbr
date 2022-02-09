mod mbc1;

use std::{cell::RefCell, rc::Rc};

use crate::{
    rom::{self, Rom},
    util::Ref,
};

pub trait Mbc {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, _addr: u16, _data: u8) {}
    fn backup_ram(&self) -> Option<&[u8]> {
        None
    }
}

struct NullMbc {
    rom: Ref<Rom>,
}

impl NullMbc {
    fn new(rom: &Ref<Rom>, _backup_ram: Option<Vec<u8>>) -> Self {
        assert_eq!(
            rom.borrow().rom_size,
            32 * 1024,
            "ROM only cartridge should be 32KiB"
        );
        assert_eq!(
            rom.borrow().ram_size,
            0,
            "Currently ROM+RAM cartridge not supported"
        );

        Self {
            rom: Ref::clone(rom),
        }
    }
}

impl Mbc for NullMbc {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3fff => self.rom.borrow().data[addr as usize],
            0x4000..=0x7fff => self.rom.borrow().data[addr as usize],
            _ => unreachable!("{:04X}", addr),
        }
    }
}

pub fn create_mbc(rom: &Ref<Rom>, backup_ram: Option<Vec<u8>>) -> Ref<dyn Mbc> {
    pub fn wrap_ref<T: Mbc + 'static>(v: T) -> Ref<dyn Mbc> {
        Ref(Rc::new(RefCell::new(v)))
    }

    let cart_type = rom.borrow().cartridge_type.clone();
    match cart_type.mbc {
        None => wrap_ref(NullMbc::new(rom, backup_ram)),
        Some(rom::Mbc::Mbc1) => wrap_ref(mbc1::Mbc1::new(rom, backup_ram)),
        Some(mbc) => todo!("{} is currently unsupported", mbc),
    }
}
