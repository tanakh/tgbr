mod mbc1;

use serde::Serialize;

use crate::{
    rom::{self, Rom},
    util::Ref,
};

#[derive(Serialize)]
pub struct NullMbc {
    #[serde(skip)]
    rom: Ref<Rom>,
}

impl NullMbc {
    pub fn new(rom: &Ref<Rom>, _backup_ram: Option<Vec<u8>>) -> Self {
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

    pub fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3fff => self.rom.borrow().data[addr as usize],
            0x4000..=0x7fff => self.rom.borrow().data[addr as usize],
            _ => unreachable!("{:04X}", addr),
        }
    }

    pub fn write(&mut self, _addr: u16, _data: u8) {}

    pub fn backup_ram(&self) -> Option<&[u8]> {
        None
    }
}

macro_rules! def_mbc {
    ($($id:ident => $ty:ty,)*) => {
        #[derive(Serialize)]
        pub enum Mbc {
            NullMbc(NullMbc),
            $(
                $id($ty),
            )*
        }

        impl Mbc {
            pub fn read(&mut self, addr: u16) -> u8 {
                match self {
                    Mbc::NullMbc(mbc) => mbc.read(addr),
                    $(
                        Mbc::$id(mbc) => mbc.read(addr),
                    )*
                }
            }

            pub fn write(&mut self, addr: u16, data: u8) {
                match self {
                    Mbc::NullMbc(mbc) => mbc.write(addr, data),
                    $(
                        Mbc::$id(mbc) => mbc.write(addr, data),
                    )*
                }
            }

            pub fn backup_ram(&self) -> Option<&[u8]> {
                match self {
                    Mbc::NullMbc(_) => None,
                    $(
                        Mbc::$id(mbc) => mbc.backup_ram(),
                    )*
                }
            }
        }

        pub fn create_mbc(rom: &Ref<Rom>, backup_ram: Option<Vec<u8>>) -> Mbc {
            let cart_type = rom.borrow().cartridge_type.clone();
            match cart_type.mbc {
                None => Mbc::NullMbc(NullMbc::new(rom, backup_ram)),
                $(
                    Some(rom::Mbc::$id) => Mbc::$id(<$ty>::new(rom, backup_ram)),
                )*
                Some(mbc) => todo!("{} is currently unsupported", mbc),
            }
        }
    }
}

def_mbc! {
    Mbc1 => mbc1::Mbc1,
}
