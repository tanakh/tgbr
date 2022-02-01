use std::fmt::Display;

use anyhow::{bail, Result};
use log::warn;
use prettytable::{cell, format, row, table};

pub struct Rom {
    pub title: String,
    pub manufacturer_code: [u8; 4],
    pub cgb_flag: CgbFlag,
    pub new_licensee_code: [u8; 2],
    pub sgb_flag: bool,
    pub cartridge_type: CartridgeType,
    pub rom_size: usize,
    pub ram_size: usize,
    pub destination_code: DestinationCode,
    pub old_licensee_code: u8,
    pub mask_rom_version: u8,
    pub header_checksum: u8,
    pub global_checksum: u16,
    pub data: Vec<u8>,
}

pub enum CgbFlag {
    NonCgb,
    SupportCgb,
    OnlyCgb,
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
        };
        write!(f, "{s}")
    }
}

impl CartridgeType {
    fn with_mbc(mbc: Mbc) -> Self {
        Self {
            mbc: Some(mbc),
            ..Default::default()
        }
    }

    fn from_code(code: u8) -> Self {
        use Mbc::*;
        match code {
            0x00 => Self::default(),
            0x01 => Self::with_mbc(Mbc1),
            0x02 => Self::with_mbc(Mbc1).with_ram(),
            0x03 => Self::with_mbc(Mbc1).with_ram().with_battery(),
            0x05 => Self::with_mbc(Mbc2),
            0x06 => Self::with_mbc(Mbc2).with_ram(),
            0x08 => Self::default().with_ram(),
            0x09 => Self::default().with_ram().with_battery(),
            0x0B => Self::with_mbc(Mmm01),
            0x0C => Self::with_mbc(Mmm01).with_ram(),
            0x0D => Self::with_mbc(Mmm01).with_ram().with_battery(),
            0x0F => Self::with_mbc(Mbc3).with_timer().with_battery(),
            0x10 => Self::with_mbc(Mbc3).with_timer().with_ram().with_battery(),
            0x11 => Self::with_mbc(Mbc3),
            0x12 => Self::with_mbc(Mbc3).with_ram(),
            0x13 => Self::with_mbc(Mbc3).with_ram().with_battery(),
            0x19 => Self::with_mbc(Mbc5),
            0x1A => Self::with_mbc(Mbc5).with_ram(),
            0x1B => Self::with_mbc(Mbc5).with_ram().with_battery(),
            0x1C => Self::with_mbc(Mbc5).with_rumble(),
            0x1D => Self::with_mbc(Mbc5).with_rumble().with_ram(),
            0x1E => Self::with_mbc(Mbc5).with_rumble().with_ram().with_battery(),
            0x20 => Self::with_mbc(Mbc6),
            0x22 => Self::with_mbc(Mbc7)
                .with_sensor()
                .with_rumble()
                .with_ram()
                .with_battery(),
            _ => panic!("Unknown cartridge type: 0x{code:02x}"),
        }
    }

    fn with_ram(mut self) -> Self {
        self.has_ram = true;
        self
    }
    fn with_battery(mut self) -> Self {
        self.has_ram = true;
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
}

impl Display for CartridgeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mbc = self
            .mbc
            .as_ref()
            .map_or_else(|| "ROM".to_string(), |mbc| mbc.to_string());
        write!(
            f,
            "{mbc}{}{}{}{}{}",
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

        let title = String::from_utf8(
            header[0x34..=0x43]
                .iter()
                .cloned()
                .take_while(|c| *c != 0)
                .collect::<Vec<_>>(),
        )?;

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
            v => bail!("Invalid SGB flag: ${v:02X}"),
        };

        let cartridge_type = CartridgeType::from_code(header[0x47]);

        let rom_size = match header[0x48] {
            n @ (0x00..=0x08) => (32 * 1024) << n,
            n => bail!("Invalid ROM size: ${n:02X}"),
        };

        if bytes.len() != rom_size {
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
        if mask_rom_version != 0x00 {
            bail!("Invalid mask ROM version: ${mask_rom_version:02X}");
        }

        let header_checksum = header[0x4d];

        let mut x = 0_u8;
        for i in 0x34..=0x4c {
            x = x.wrapping_sub(header[i]).wrapping_sub(1);
        }

        if x != header_checksum {
            warn!("Invalid header checksum: checksum in ROM is ${header_checksum:02X}, but calculated checksum is ${x:02X}");
        }

        let global_checksum = (header[0x4e] as u16) << 8 | header[0x4f] as u16;

        let mut check_sum = 0_u16;
        for i in 0..bytes.len() {
            if !(0x14e..=0x14f).contains(&i) {
                check_sum = check_sum.wrapping_add(bytes[i] as u16);
            }
        }

        if global_checksum != check_sum {
            warn!("Invalid global checksum: checksum in ROM is ${global_checksum:04X}, but calculated checksum is ${check_sum:04X}");
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
            global_checksum,
            data: bytes.to_vec(),
        })
    }

    pub fn info(&self) {
        let to_si = |x| bytesize::ByteSize(x as _).to_string_as(true);

        let mut table = table! {
            [ "Title", self.title ],
            [ "Manufacturer Code", {
                let c = self.manufacturer_code;
                format!("{:02X} {:02X} {:02X} {:02X}", c[0], c[1], c[2], c[3])
            }],
            [ "CGB Flag", self.cgb_flag ],
            [ "New Licensee Code", format!("{:02X} {:02X}", self.new_licensee_code[0], self.new_licensee_code[1]) ],
            [ "SGB Flag", self.sgb_flag ],
            [ "Cartridge Type", self.cartridge_type ],
            [ "ROM Size",  to_si(self.rom_size) ],
            [ "RAM Size", to_si(self.ram_size) ],
            [ "Destination Code", self.destination_code ],
            [ "Old Licensee Code", self.old_licensee_code ],
            [ "Mask ROM Version", self.mask_rom_version ],
            [ "Header Checksum", format!("{:02X}", self.header_checksum) ],
            [ "Global Checksum", format!("{:04X}", self.global_checksum) ]
        };

        table.set_titles(row!["ROM File Info"]);
        table.set_format(*format::consts::FORMAT_NO_LINESEP_WITH_TITLE);
        println!("\n{}", table);
    }
}
