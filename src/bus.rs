use log::{trace, warn};

use crate::{io::Io, mbc::Mbc, util::Ref};

pub struct Bus {
    ram: [u8; 0x2000],
    vram: Ref<Vec<u8>>,
    oam: Ref<Vec<u8>>,
    oam_lock: Ref<bool>,
    hiram: [u8; 0x7F],
    boot_rom: [u8; 0x100],
    map_boot_rom: bool,
    mbc: Ref<dyn Mbc>,
    io: Ref<Io>,
    dma: Dma,
}

#[derive(Default)]
struct Dma {
    source: u8,
    pos: u8,
    enabled: bool,
    delay: usize,
}

impl Bus {
    pub fn new(
        mbc: &Ref<dyn Mbc>,
        vram: &Ref<Vec<u8>>,
        oam: &Ref<Vec<u8>>,
        oam_lock: &Ref<bool>,
        boot_rom: &Option<Vec<u8>>,
        io: &Ref<Io>,
    ) -> Self {
        Self {
            ram: [0; 0x2000],
            vram: Ref::clone(vram),
            oam: Ref::clone(oam),
            oam_lock: Ref::clone(oam_lock),
            hiram: [0; 0x7F],
            boot_rom: boot_rom
                .as_ref()
                .map_or_else(|| [0; 0x100], |r| r.as_slice().try_into().unwrap()),
            map_boot_rom: boot_rom.is_some(),
            mbc: Ref::clone(mbc),
            io: Ref::clone(io),
            dma: Dma::default(),
        }
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        let data = self.read_(addr);
        trace!("<-- Read:  ${addr:04X} = ${data:02X}");
        data
    }

    pub fn read_immutable(&mut self, addr: u16) -> Option<u8> {
        match addr {
            0xff00..=0xff7f | 0xffff => None,
            _ => Some(self.read_(addr)),
        }
    }

    fn read_(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x00FF => {
                if self.map_boot_rom {
                    self.boot_rom[addr as usize]
                } else {
                    self.mbc.borrow_mut().read(addr)
                }
            }
            0x0100..=0x7fff => self.mbc.borrow_mut().read(addr),
            0x8000..=0x9fff => self.vram.borrow()[(addr & 0x1fff) as usize],
            0xa000..=0xbfff => self.mbc.borrow_mut().read(addr),
            0xc000..=0xfdff => self.ram[(addr & 0x1fff) as usize],
            0xfe00..=0xfe9f => {
                if !*self.oam_lock.borrow() && !self.dma.enabled {
                    self.oam.borrow_mut()[(addr & 0xff) as usize]
                } else {
                    !0
                }
            }
            0xfea0..=0xfeff => todo!("Read from Unusable address: ${addr:04x}"),
            0xff00..=0xff7f => {
                if addr == 0xff46 {
                    // DMA
                    self.dma.source
                } else {
                    self.io.borrow_mut().read(addr)
                }
            }
            0xff80..=0xfffe => self.hiram[(addr & 0x7f) as usize],
            0xffff => self.io.borrow_mut().read(addr),
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        trace!("--> Write: ${addr:04X} = ${data:02X}");
        if self.dma.enabled {
            if matches!(addr, 0xff80..=0xfffe) {
                self.hiram[(addr & 0x7f) as usize] = data;
            }
            return;
        }
        match addr {
            0x0000..=0x7fff => self.mbc.borrow_mut().write(addr, data),
            0x8000..=0x9fff => self.vram.borrow_mut()[(addr & 0x1fff) as usize] = data,
            0xa000..=0xbfff => self.mbc.borrow_mut().write(addr, data),
            0xc000..=0xfdff => self.ram[(addr & 0x1fff) as usize] = data,
            0xfe00..=0xfe9f => {
                if !*self.oam_lock.borrow() && !self.dma.enabled {
                    self.oam.borrow_mut()[(addr & 0xff) as usize] = data
                }
            }
            0xfea0..=0xfeff => warn!("Write to Unusable address: ${addr:04X} = ${data:02X}"),

            0xff46 => {
                // DMA
                self.dma.source = data;
                self.dma.pos = 0;
                self.dma.enabled = false;
                self.dma.delay = 2;
            }
            0xff50 => self.map_boot_rom = data == 0, // BANK
            0xff00..=0xff7f => self.io.borrow_mut().write(addr, data),
            0xff80..=0xfffe => self.hiram[(addr & 0x7f) as usize] = data,
            0xffff => self.io.borrow_mut().write(addr, data),
        };
    }

    pub fn tick(&mut self) {
        self.process_dma();
        self.io.borrow_mut().tick();
    }

    fn process_dma(&mut self) {
        if self.dma.delay > 0 {
            self.dma.delay -= 1;
            if self.dma.delay == 0 {
                self.dma.enabled = true;
            }
            return;
        }
        if !self.dma.enabled {
            return;
        }
        log::trace!(
            "<-> DMA:   ${:02X}{:02X} -> $FE{:02X}",
            self.dma.source,
            self.dma.pos,
            self.dma.pos
        );
        let data = self.read_((self.dma.source as u16) << 8 | self.dma.pos as u16);
        self.oam.borrow_mut()[self.dma.pos as usize] = data;
        self.dma.pos += 1;
        if self.dma.pos == 0xA0 {
            self.dma.enabled = false;
        }
    }
}
