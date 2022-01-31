use bitvec::prelude::*;
use log::warn;

#[derive(Default)]
pub struct Ppu {
    ppu_enable: bool,                     // 0=off, 1=on
    window_tile_map_select: bool,         // 0=9800-9BFF, 1=9C00-9FFF
    window_enable: bool,                  // 0=off, 1=on
    bg_and_window_tile_data_select: bool, // 0=8800-97FF, 1=8000-8FFF
    bg_tile_map_select: bool,             // 0=9800-9BFF, 1=9C00-9FFF
    obj_size: bool,                       // 0=8x8, 1=8x16
    obj_enable: bool,                     // 0=off, 1=on
    bg_and_window_enable: bool,           // 0=off, 1=on

    lyc_interrupt_enable: bool,
    oam_interrupt_enable: bool,
    vblank_interrupt_enable: bool,
    hblank_interrupt_enable: bool,
    mode: u8,

    scy: u8,
    scx: u8,

    ly: u8,
    lyc: u8,

    wy: u8,
    wx: u8,

    lx: u8,
    frame: u64,
}

impl Ppu {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn tick(&mut self) {
        todo!()
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        match addr & 0xff {
            // LCDC: LCD Control (R/W)
            0x40 => {
                let mut ret = 0;
                let v = ret.view_bits_mut::<Lsb0>();
                v.set(0, self.ppu_enable);
                v.set(1, self.window_tile_map_select);
                v.set(2, self.window_enable);
                v.set(3, self.bg_and_window_tile_data_select);
                v.set(4, self.bg_tile_map_select);
                v.set(5, self.obj_size);
                v.set(6, self.obj_enable);
                v.set(7, self.bg_and_window_enable);
                ret
            }
            // STAT: LCDC Status (R/W)
            0x41 => {
                let mut ret = 0;
                let v = ret.view_bits_mut::<Lsb0>();
                v.set(6, self.lyc_interrupt_enable);
                v.set(5, self.oam_interrupt_enable);
                v.set(4, self.vblank_interrupt_enable);
                v.set(3, self.hblank_interrupt_enable);
                v.set(2, self.lyc == self.ly);
                v[0..=1].store(self.mode);
                ret
            }
            // SCY: Scroll Y (R/W)
            0x42 => self.scy,
            // SCX: Scroll X (R/W)
            0x43 => self.scx,
            // LY: LCDC Y-Coordinate (R)
            0x44 => self.ly,
            // LYC: LY Compare (R/W)
            0x45 => self.lyc,
            // WY: Window Y Position (R/W)
            0x4a => self.wy,
            // WX: Window X Position (R/W)
            0x4b => self.wx,
            _ => {
                todo!("Read from LCD I/O: ${addr:04X}");
                // warn!("Unusable read from I/O: ${addr:04X}");
                0
            }
        }
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        match addr & 0xff {
            // LCDC: LCD Control (R/W)
            0x40 => {
                let v = data.view_bits::<Lsb0>();
                self.ppu_enable = v[7];
                self.window_tile_map_select = v[6];
                self.window_enable = v[5];
                self.bg_and_window_tile_data_select = v[4];
                self.bg_tile_map_select = v[3];
                self.obj_size = v[2];
                self.obj_enable = v[1];
                self.bg_and_window_enable = v[0];
            }
            // STAT: LCDC Status (R/W)
            0x41 => {
                let v = data.view_bits::<Lsb0>();
                self.lyc_interrupt_enable = v[6];
                self.oam_interrupt_enable = v[5];
                self.vblank_interrupt_enable = v[4];
                self.hblank_interrupt_enable = v[3];
            }
            // SCY: Scroll Y (R/W)
            0x42 => self.scy = data,
            // SCX: Scroll X (R/W)
            0x43 => self.scx = data,
            // LYC: LY Compare (R/W)
            0x45 => self.lyc = data,
            // WY: Window Y Position (R/W)
            0x4a => self.wy = data,
            // WX: Window X Position (R/W)
            0x4b => self.wx = data,
            _ => warn!("Unusable write to I/O: ${addr:04X} = ${data:02X}"),
        }
    }
}
