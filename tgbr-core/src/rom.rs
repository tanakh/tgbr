use anyhow::{bail, Result};
use log::warn;
use std::fmt::Display;

use crate::util::to_si_bytesize;

#[derive(Default)]
pub struct Rom {
    pub title: String,
    pub manufacturer_code: [u8; 4],
    pub cgb_flag: CgbFlag,
    pub new_licensee_code: [u8; 2],
    pub sgb_flag: bool,
    pub cartridge_type: CartridgeType,
    pub rom_size: u64,
    pub ram_size: u64,
    pub destination_code: DestinationCode,
    pub old_licensee_code: u8,
    pub mask_rom_version: u8,
    pub header_checksum: u8,
    pub header_checksum_ok: bool,
    pub global_checksum: u16,
    pub global_checksum_ok: bool,
    pub data: Vec<u8>,
}

pub enum CgbFlag {
    NonCgb,
    SupportCgb,
    OnlyCgb,
}

impl Default for CgbFlag {
    fn default() -> Self {
        CgbFlag::NonCgb
    }
}

impl Display for CgbFlag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CgbFlag::NonCgb => write!(f, "Non-CGB"),
            CgbFlag::SupportCgb => write!(f, "Support CGB"),
            CgbFlag::OnlyCgb => write!(f, "Only CGB"),
        }
    }
}

pub enum DestinationCode {
    Japanese,
    NonJapanese,
}

impl Default for DestinationCode {
    fn default() -> Self {
        DestinationCode::Japanese
    }
}

impl Display for DestinationCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DestinationCode::Japanese => write!(f, "Japanese"),
            DestinationCode::NonJapanese => write!(f, "Non-Japanese"),
        }
    }
}

#[derive(Default, Clone)]
pub struct CartridgeType {
    pub code: u8,
    pub mbc: Option<Mbc>,
    pub has_ram: bool,
    pub has_battery: bool,
    pub has_timer: bool,
    pub has_rumble: bool,
    pub has_sensor: bool,
}

#[derive(Clone)]
pub enum Mbc {
    Mbc1,
    Mbc2,
    Mmm01,
    Mbc3,
    Mbc5,
    Mbc6,
    Mbc7,
    HuC1,
    HuC3,
}

impl Display for Mbc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Mbc::Mbc1 => "MBC1",
            Mbc::Mbc2 => "MBC2",
            Mbc::Mmm01 => "MMM01",
            Mbc::Mbc3 => "MBC3",
            Mbc::Mbc5 => "MBC5",
            Mbc::Mbc6 => "MBC6",
            Mbc::Mbc7 => "MBC7",
            Mbc::HuC1 => "HuC1",
            Mbc::HuC3 => "HuC3",
        };
        write!(f, "{s}")
    }
}

impl CartridgeType {
    pub fn from_code(code: u8) -> Result<Self> {
        let ret = Self {
            code,
            ..Default::default()
        };

        use Mbc::*;
        Ok(match code {
            0x00 => ret,
            0x01 => ret.with_mbc(Mbc1),
            0x02 => ret.with_mbc(Mbc1).with_ram(),
            0x03 => ret.with_mbc(Mbc1).with_ram().with_battery(),
            0x05 => ret.with_mbc(Mbc2),
            0x06 => ret.with_mbc(Mbc2).with_battery(),
            0x08 => ret.with_ram(),
            0x09 => ret.with_ram().with_battery(),
            0x0B => ret.with_mbc(Mmm01),
            0x0C => ret.with_mbc(Mmm01).with_ram(),
            0x0D => ret.with_mbc(Mmm01).with_ram().with_battery(),
            0x0F => ret.with_mbc(Mbc3).with_timer().with_battery(),
            0x10 => ret.with_mbc(Mbc3).with_timer().with_ram().with_battery(),
            0x11 => ret.with_mbc(Mbc3),
            0x12 => ret.with_mbc(Mbc3).with_ram(),
            0x13 => ret.with_mbc(Mbc3).with_ram().with_battery(),
            0x19 => ret.with_mbc(Mbc5),
            0x1A => ret.with_mbc(Mbc5).with_ram(),
            0x1B => ret.with_mbc(Mbc5).with_ram().with_battery(),
            0x1C => ret.with_mbc(Mbc5).with_rumble(),
            0x1D => ret.with_mbc(Mbc5).with_rumble().with_ram(),
            0x1E => ret.with_mbc(Mbc5).with_rumble().with_ram().with_battery(),
            0x20 => ret.with_mbc(Mbc6),
            0x22 => ret
                .with_mbc(Mbc7)
                .with_sensor()
                .with_rumble()
                .with_ram()
                .with_battery(),
            0xFE => ret.with_mbc(HuC3),
            0xFF => ret.with_mbc(HuC1).with_ram().with_battery(),
            _ => bail!("Unknown cartridge type: ${code:02X}"),
        })
    }

    fn with_mbc(mut self, mbc: Mbc) -> Self {
        self.mbc = Some(mbc);
        self
    }

    fn with_ram(mut self) -> Self {
        self.has_ram = true;
        self
    }
    fn with_battery(mut self) -> Self {
        self.has_battery = true;
        self
    }
    fn with_timer(mut self) -> Self {
        self.has_timer = true;
        self
    }
    fn with_rumble(mut self) -> Self {
        self.has_rumble = true;
        self
    }
    fn with_sensor(mut self) -> Self {
        self.has_sensor = true;
        self
    }

    pub fn has_internal_ram(&self) -> bool {
        match &self.mbc {
            Some(Mbc::Mbc2) => true,
            _ => false,
        }
    }
}

impl Display for CartridgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mbc = self
            .mbc
            .as_ref()
            .map_or_else(|| "ROM".to_string(), |mbc| mbc.to_string());
        write!(
            f,
            "{:02X}: {mbc}{}{}{}{}{}",
            self.code,
            if self.has_ram { "+RAM" } else { "" },
            if self.has_battery { "+BATTERY" } else { "" },
            if self.has_timer { "+TIMER" } else { "" },
            if self.has_rumble { "+RUMBLE" } else { "" },
            if self.has_sensor { "+SENSOR" } else { "" },
        )
    }
}

impl Rom {
    pub fn from_bytes(bytes: &[u8]) -> Result<Rom> {
        let header = &bytes[0x100..=0x14f];

        let title = String::from_utf8_lossy(&header[0x34..=0x43]).to_string();

        let manufacturer_code: [u8; 4] = header[0x3f..=0x42].try_into()?;
        let cgb_flag = match header[0x43] {
            0x80 => CgbFlag::SupportCgb,
            0xC0 => CgbFlag::OnlyCgb,
            _ => CgbFlag::NonCgb,
        };

        let new_licensee_code: [u8; 2] = header[0x44..=0x45].try_into()?;
        let sgb_flag = match header[0x46] {
            0x03 => true,
            0x00 => false,
            v => {
                warn!("Invalid SGB flag: ${v:02X}");
                false
            }
        };

        let cartridge_type = CartridgeType::from_code(header[0x47])?;

        let rom_size: u64 = match header[0x48] {
            n @ (0x00..=0x08) => (32 * 1024) << n,
            n => bail!("Invalid ROM size: ${n:02X}"),
        };

        if bytes.len() as u64 != rom_size {
            bail!(
                "ROM size mismatch: header expected {rom_size}, but actual size is  {}",
                bytes.len()
            );
        }

        let ram_size = match header[0x49] {
            0 => 0,
            2 => 8 * 1024,
            3 => 32 * 1024,
            4 => 128 * 1024,
            5 => 64 * 1024,
            n => bail!("Invalid RAM size: ${n:02X}"),
        };

        let destination_code = match header[0x4a] {
            0x00 => DestinationCode::Japanese,
            0x01 => DestinationCode::NonJapanese,
            v => bail!("Invalid destination code: ${v:02X}"),
        };

        // Specifies the games company/publisher code in range $00-FF.
        // A value of $33 signals that the New Licensee Code (in header bytes $0144-0145) is used instead.
        let old_licensee_code = header[0x4b];

        let mask_rom_version = header[0x4c];

        let header_checksum = header[0x4d];

        let mut header_checksum_calc = 0_u8;
        for i in 0x34..=0x4c {
            header_checksum_calc = header_checksum_calc.wrapping_sub(header[i]).wrapping_sub(1);
        }

        if header_checksum_calc != header_checksum {
            warn!("Invalid header checksum: checksum in ROM is ${header_checksum:02X}, but calculated checksum is ${header_checksum_calc:02X}");
        }

        let global_checksum = (header[0x4e] as u16) << 8 | header[0x4f] as u16;

        let mut global_checksum_calc = 0_u16;
        for i in 0..bytes.len() {
            if !(0x14e..=0x14f).contains(&i) {
                global_checksum_calc = global_checksum_calc.wrapping_add(bytes[i] as u16);
            }
        }

        if global_checksum_calc != global_checksum {
            warn!("Invalid global checksum: checksum in ROM is ${global_checksum:04X}, but calculated checksum is ${global_checksum_calc:04X}");
        }

        Ok(Rom {
            title,
            manufacturer_code,
            cgb_flag,
            new_licensee_code,
            sgb_flag,
            cartridge_type,
            rom_size,
            ram_size,
            destination_code,
            old_licensee_code,
            mask_rom_version,
            header_checksum,
            header_checksum_ok: header_checksum_calc == header_checksum,
            global_checksum,
            global_checksum_ok: global_checksum_calc == global_checksum,
            data: bytes.to_vec(),
        })
    }

    pub fn info(&self) -> Vec<(&str, String)> {
        vec![
            ("Title", self.title.to_owned()),
            ("Manufacturer Code", {
                let c = self.manufacturer_code;
                format!("{:02X} {:02X} {:02X} {:02X}", c[0], c[1], c[2], c[3])
            }),
            ("CGB Flag", self.cgb_flag.to_string()),
            (
                "New Licensee Code",
                format!(
                    "{:02X} {:02X}",
                    self.new_licensee_code[0], self.new_licensee_code[1]
                ),
            ),
            ("Suport SGB", self.sgb_flag.to_string()),
            ("Cartridge Type", self.cartridge_type.to_string()),
            ("ROM Size", to_si_bytesize(self.rom_size)),
            ("RAM Size", to_si_bytesize(self.ram_size)),
            ("Destination Code", self.destination_code.to_string()),
            ("Old Licensee Code", self.old_licensee_code.to_string()),
            ("Mask ROM Version", self.mask_rom_version.to_string()),
            (
                "Header Checksum",
                format!(
                    "{:02X} ({})",
                    self.header_checksum,
                    if self.header_checksum_ok {
                        "Good"
                    } else {
                        "Bad"
                    }
                ),
            ),
            (
                "Global Checksum",
                format!(
                    "{:04X} ({})",
                    self.global_checksum,
                    if self.global_checksum_ok {
                        "Good"
                    } else {
                        "Bad"
                    }
                ),
            ),
        ]
    }
}
