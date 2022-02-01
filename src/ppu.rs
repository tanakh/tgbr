use bitvec::prelude::*;
use log::warn;

use crate::{
    consts::{
        DOTS_PER_LINE, INT_LCD_STAT, INT_VBLANK, LINES_PER_FRAME, SCREEN_WIDTH, VISIBLE_RANGE,
    },
    interface::{Color, FrameBuffer},
    util::Ref,
};

const DMG_PALETTE: [Color; 4] = [
    Color::new(255, 255, 255),
    Color::new(170, 170, 170),
    Color::new(85, 85, 85),
    Color::new(0, 0, 0),
];

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

    scroll_y: u8,
    scroll_x: u8,
    window_y: u8,
    window_x: u8,

    bg_palette: [u8; 4],
    obj_palette: [[u8; 4]; 2],

    ly: u8,
    lyc: u8,
    lx: u64,
    frame: u64,

    line_buffer: [u8; SCREEN_WIDTH as usize],
    frame_buffer: Ref<FrameBuffer>,

    vram: Ref<Vec<u8>>,
    oam: Ref<Vec<u8>>,
    interrupt_flag: Ref<u8>,
}

impl Ppu {
    pub fn new(
        vram: &Ref<Vec<u8>>,
        oam: &Ref<Vec<u8>>,
        interrupt_flag: &Ref<u8>,
        frame_buffer: &Ref<FrameBuffer>,
    ) -> Self {
        Self {
            ppu_enable: false,
            window_tile_map_select: false,
            window_enable: false,
            bg_and_window_tile_data_select: false,
            bg_tile_map_select: false,
            obj_size: false,
            obj_enable: false,
            bg_and_window_enable: false,
            lyc_interrupt_enable: false,
            oam_interrupt_enable: false,
            vblank_interrupt_enable: false,
            hblank_interrupt_enable: false,
            mode: 0,
            scroll_y: 0,
            scroll_x: 0,
            ly: 0,
            lyc: 0,
            window_y: 0,
            window_x: 0,
            bg_palette: [0; 4],
            obj_palette: [[0; 4]; 2],
            lx: 0,
            frame: 0,
            line_buffer: [0; SCREEN_WIDTH as usize],
            frame_buffer: Ref::clone(frame_buffer),
            vram: Ref::clone(vram),
            oam: Ref::clone(oam),
            interrupt_flag: Ref::clone(interrupt_flag),
        }
    }

    pub fn tick(&mut self) {
        if VISIBLE_RANGE.contains(&(self.ly as u64)) {
            if self.lx == 0 {
                self.render_line();
            }

            if self.lx < 80 {
                if self.mode != 2 && self.oam_interrupt_enable {
                    *self.interrupt_flag.borrow_mut() |= INT_LCD_STAT;
                }
                self.mode = 2;
            } else if self.lx < 80 + 172 {
                // FIXME: mode 3 extends at most 289 dots
                if self.mode != 3 && self.hblank_interrupt_enable {
                    *self.interrupt_flag.borrow_mut() |= INT_LCD_STAT;
                }
                self.mode = 3;
            } else {
                self.mode = 0;
            }
        } else {
            if self.mode != 1 {
                *self.interrupt_flag.borrow_mut() |= INT_VBLANK;
                if self.vblank_interrupt_enable {
                    *self.interrupt_flag.borrow_mut() |= INT_LCD_STAT;
                }
            }
            self.mode = 1;
        }

        self.lx += 1;
        if self.lx == DOTS_PER_LINE {
            self.lx = 0;

            self.ly += 1;
            if self.ly == LINES_PER_FRAME as u8 {
                self.ly = 0;
                self.frame += 1;

                if self.ly == self.lyc && self.lyc_interrupt_enable {
                    *self.interrupt_flag.borrow_mut() |= INT_LCD_STAT;
                }
            }
        }
    }

    pub fn frame(&self) -> u64 {
        self.frame
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
            0x42 => self.scroll_y,
            // SCX: Scroll X (R/W)
            0x43 => self.scroll_x,
            // LY: LCDC Y-Coordinate (R)
            0x44 => self.ly,
            // LYC: LY Compare (R/W)
            0x45 => self.lyc,
            // BGP: BG Palette Data (R/W)
            0x47 => {
                let mut ret = 0;
                let v = ret.view_bits_mut::<Lsb0>();
                v[0..=1].store(self.bg_palette[0]);
                v[2..=3].store(self.bg_palette[1]);
                v[4..=5].store(self.bg_palette[2]);
                v[6..=7].store(self.bg_palette[3]);
                ret
            }
            // OBP0/1: Object Palette 0/1 Data (R/W)
            0x48 | 0x49 => {
                let ix = (addr & 0x1) as usize;
                let mut ret = 0;
                let v = ret.view_bits_mut::<Lsb0>();
                v[0..=1].store(self.obj_palette[ix][0]);
                v[2..=3].store(self.obj_palette[ix][1]);
                v[4..=5].store(self.obj_palette[ix][2]);
                v[6..=7].store(self.obj_palette[ix][3]);
                ret
            }
            // WY: Window Y Position (R/W)
            0x4a => self.window_y,
            // WX: Window X Position (R/W)
            0x4b => self.window_x,
            _ => {
                todo!("Read from LCD I/O: ${addr:04X}");
                // warn!("Unusable read from I/O: ${addr:04X}");
                // 0
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
            0x42 => self.scroll_y = data,
            // SCX: Scroll X (R/W)
            0x43 => self.scroll_x = data,
            // LYC: LY Compare (R/W)
            0x45 => self.lyc = data,
            // DMA: DMA Transfer and Start Address (W)
            0x46 => {
                todo!("DMA")
            }
            // BGP: BG Palette Data (R/W)
            0x47 => {
                let v = data.view_bits::<Lsb0>();
                self.bg_palette[0] = v[0..=1].load();
                self.bg_palette[1] = v[2..=3].load();
                self.bg_palette[2] = v[4..=5].load();
                self.bg_palette[3] = v[6..=7].load();
            }
            // OBP0/1: Object Palette 0/1 Data (R/W)
            0x48 | 0x49 => {
                let ix = (addr & 0x1) as usize;
                let v = data.view_bits::<Lsb0>();
                self.obj_palette[ix][0] = v[0..=1].load();
                self.obj_palette[ix][1] = v[2..=3].load();
                self.obj_palette[ix][2] = v[4..=5].load();
                self.obj_palette[ix][3] = v[6..=7].load();
            }
            // WY: Window Y Position (R/W)
            0x4a => self.window_y = data,
            // WX: Window X Position (R/W)
            0x4b => self.window_x = data,
            _ => warn!("Unusable write to I/O: ${addr:04X} = ${data:02X}"),
        }
    }
}

impl Ppu {
    fn render_line(&mut self) {
        self.line_buffer.fill(0);
        if self.ppu_enable && self.bg_and_window_enable {
            self.render_bg_line();
        }
        if self.ppu_enable && self.obj_enable {
            self.render_obj_line();
        }
        let mut fb = self.frame_buffer.borrow_mut();
        for x in 0..SCREEN_WIDTH as usize {
            let c = self.line_buffer[x];
            fb.set(x, self.ly as _, DMG_PALETTE[(c & 3) as usize])
        }
    }

    fn render_bg_line(&mut self) {
        let vram = self.vram.borrow();
        let tile_data = if self.bg_and_window_tile_data_select {
            0x8000 - 0x8000
        } else {
            0x9000 - 0x8000
        };
        let tile_map = if self.bg_tile_map_select {
            0x9c00 - 0x8000
        } else {
            0x9800 - 0x8000
        };

        let y = self.ly.wrapping_add(self.scroll_y);

        for sx in 0..SCREEN_WIDTH as u8 {
            let x = sx.wrapping_add(self.scroll_x);

            let tile_x = x as usize / 8;
            let tile_y = y as usize / 8;
            let ofs_x = x as usize % 8;
            let ofs_y = y as usize % 8;

            let tile_ix = vram[tile_map + tile_y * 32 + tile_x];

            let mut tile_addr = tile_data + (tile_ix as usize * 16);
            if tile_addr >= 0x1800 {
                tile_addr -= 0x1000;
            }

            let tile_data = &vram[tile_addr..tile_addr + 16];
            let lo = tile_data[ofs_y * 2];
            let hi = tile_data[ofs_y * 2 + 1];

            let b = (lo >> (7 - ofs_x)) & 1 | ((hi >> (7 - ofs_x)) & 1) << 1;

            self.line_buffer[sx as usize] = self.bg_palette[b as usize];
        }
    }

    fn render_obj_line(&mut self) {
        todo!()
    }
}
