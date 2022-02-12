mod mbc1;

use serde::{Deserialize, Serialize};

use crate::{
    context,
    rom::{self, Rom},
    util::trait_alias,
};

trait_alias!(pub trait Context = context::Rom);

#[derive(Serialize, Deserialize)]
pub struct NullMbc {}

impl NullMbc {
    pub fn new(rom: &Rom, _backup_ram: Option<Vec<u8>>) -> Self {
        assert_eq!(
            rom.rom_size,
            32 * 1024,
            "ROM only cartridge should be 32KiB"
        );
        assert_eq!(rom.ram_size, 0, "Currently ROM+RAM cartridge not supported");

        Self {}
    }

    pub fn read(&mut self, ctx: &mut impl Context, addr: u16) -> u8 {
        match addr {
            0x0000..=0x3fff => ctx.rom().data[addr as usize],
            0x4000..=0x7fff => ctx.rom().data[addr as usize],
            _ => unreachable!("{:04X}", addr),
        }
    }

    pub fn write(&mut self, _ctx: &mut impl Context, _addr: u16, _data: u8) {}

    pub fn backup_ram(&self, _ctx: &mut impl Context) -> Option<&[u8]> {
        None
    }
}

macro_rules! def_mbc {
    ($($id:ident => $ty:ty,)*) => {
        #[derive(Serialize, Deserialize)]
        pub enum Mbc {
            NullMbc(NullMbc),
            $(
                $id($ty),
            )*
        }

        impl Mbc {
            pub fn read(&mut self,  ctx: &mut impl Context, addr: u16) -> u8 {
                match self {
                    Mbc::NullMbc(mbc) => mbc.read(ctx, addr),
                    $(
                        Mbc::$id(mbc) => mbc.read(ctx, addr),
                    )*
                }
            }

            pub fn write(&mut self, ctx: &mut impl Context, addr: u16, data: u8) {
                match self {
                    Mbc::NullMbc(mbc) => mbc.write(ctx, addr, data),
                    $(
                        Mbc::$id(mbc) => mbc.write(ctx, addr, data),
                    )*
                }
            }

            pub fn backup_ram(&self, ctx: &mut impl Context) -> Option<&[u8]> {
                match self {
                    Mbc::NullMbc(mbc) => mbc.backup_ram(ctx),
                    $(
                        Mbc::$id(mbc) => mbc.backup_ram(ctx),
                    )*
                }
            }
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
