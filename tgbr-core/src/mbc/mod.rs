mod mbc1;

use ambassador::{delegatable_trait, Delegate};
use serde::{Deserialize, Serialize};

use crate::{
    context,
    rom::{self, Rom},
    util::trait_alias,
};

trait_alias!(pub trait Context = context::Rom);

#[allow(unused_variables)]
#[delegatable_trait]
pub trait MbcTrait {
    fn read(&mut self, ctx: &mut impl Context, addr: u16) -> u8;
    fn write(&mut self, ctx: &mut impl Context, addr: u16, data: u8) {}
    fn backup_ram(&self, ctx: &mut impl Context) -> Option<&[u8]> {
        None
    }
}

#[derive(Serialize, Deserialize)]
pub struct NullMbc {}

impl NullMbc {
    fn new(rom: &Rom, _backup_ram: Option<Vec<u8>>) -> Self {
        assert_eq!(
            rom.rom_size,
            32 * 1024,
            "ROM only cartridge should be 32KiB"
        );
        assert_eq!(rom.ram_size, 0, "Currently ROM+RAM cartridge not supported");
        Self {}
    }
}

impl MbcTrait for NullMbc {
    fn read(&mut self, ctx: &mut impl Context, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3fff => ctx.rom().data[addr as usize],
            0x4000..=0x7fff => ctx.rom().data[addr as usize],
            _ => unreachable!("{:04X}", addr),
        }
    }
}

macro_rules! def_mbc {
    ($($id:ident => $ty:ty,)*) => {
        #[derive(Serialize, Deserialize, Delegate)]
        #[delegate(MbcTrait)]
        pub enum Mbc {
            NullMbc(NullMbc),
            $(
                $id($ty),
            )*
        }

        pub fn create_mbc(rom: &Rom, backup_ram: Option<Vec<u8>>) -> Mbc {
            let cart_type = rom.cartridge_type.clone();
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
