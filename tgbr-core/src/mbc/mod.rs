mod mbc1;
mod mbc2;
mod mbc3;
mod mbc5;

use ambassador::{delegatable_trait, Delegate};
use log::warn;
use serde::{Deserialize, Serialize};

use crate::{
    context,
    rom::{self, Rom},
    util::trait_alias,
};

trait_alias!(pub trait Context = context::Rom + context::ExternalRam);

#[allow(unused_variables)]
#[delegatable_trait]
pub trait MbcTrait {
    fn read(&mut self, ctx: &mut impl Context, addr: u16) -> u8;
    fn write(&mut self, ctx: &mut impl Context, addr: u16, data: u8) {}
    fn internal_ram(&self) -> Option<&[u8]> {
        None
    }
}

#[derive(Serialize, Deserialize)]
pub struct NullMbc {}

impl NullMbc {
    fn new(rom: &Rom) -> Self {
        assert_eq!(
            rom.rom_size,
            32 * 1024,
            "ROM only cartridge should be 32KiB"
        );
        Self {}
    }
}

impl MbcTrait for NullMbc {
    fn read(&mut self, ctx: &mut impl Context, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3FFF => ctx.rom().data[addr as usize],
            0x4000..=0x7FFF => ctx.rom().data[addr as usize],
            0xA000..=0xBFFF => ctx.external_ram()[addr as usize - 0xA000],
            _ => panic!("{addr:04X}"),
        }
    }
    fn write(&mut self, ctx: &mut impl Context, addr: u16, data: u8) {
        match addr {
            0xA000..=0xBFFF => ctx.external_ram_mut()[addr as usize - 0xA000] = data,
            _ => warn!("Invalid address write: ${addr:04X} = ${data:02X}"),
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

        pub fn create_mbc(rom: &Rom, internal_ram: Option<Vec<u8>>) -> Mbc {
            let cart_type = rom.cartridge_type.clone();
            match cart_type.mbc {
                None => Mbc::NullMbc(NullMbc::new(rom)),
                $(
                    Some(rom::Mbc::$id) => Mbc::$id(<$ty>::new(rom, internal_ram)),
                )*
                Some(mbc) => todo!("{} is currently unsupported", mbc),
            }
        }
    }
}

def_mbc! {
    Mbc1 => mbc1::Mbc1,
    Mbc2 => mbc2::Mbc2,
    Mbc3 => mbc3::Mbc3,
    Mbc5 => mbc5::Mbc5,
}
