use bitvec::prelude::*;
use log::{error, trace, warn};

use crate::{
    consts::{
        DOTS_PER_LINE, INT_LCD_STAT, INT_VBLANK, LINES_PER_FRAME, SCREEN_WIDTH, VISIBLE_RANGE,
    },
    interface::{Color, FrameBuffer},
    util::{pack, Ref},
};

const MODE_HBLANK: u8 = 0;
const MODE_VBLANK: u8 = 1;
const MODE_OAM_SEARCH: u8 = 2;
const MODE_TRANSFER: u8 = 3;

const ATTR_NONE: u8 = 0;
const ATTR_BG: u8 = 1;
const ATTR_OBJ: u8 = 2;

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
    prev_lcd_interrupt: bool,

    scroll_y: u8,
    scroll_x: u8,
    window_y: u8,
    window_x: u8,

    bg_palette: [u8; 4],
    obj_palette: [[u8; 4]; 2],

    lyc: u8,
    ly: u8,
    lx: u64,
    frame: u64,
    window_rendering_counter: u8,

    line_buffer: [u8; SCREEN_WIDTH as usize],
    line_buffer_attr: [u8; SCREEN_WIDTH as usize],
    frame_buffer: Ref<FrameBuffer>,

    vram: Ref<Vec<u8>>,
    oam: Ref<Vec<u8>>,
    oam_lock: Ref<bool>,
    interrupt_flag: Ref<u8>,

    dmg_palette: [Color; 4],
}

impl Ppu {
    pub fn new(
        vram: &Ref<Vec<u8>>,
        oam: &Ref<Vec<u8>>,
        oam_lock: &Ref<bool>,
        interrupt_flag: &Ref<u8>,
        frame_buffer: &Ref<FrameBuffer>,
        dmg_palette: &[Color; 4],
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
            prev_lcd_interrupt: false,
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
            window_rendering_counter: 0,
            line_buffer: [0; SCREEN_WIDTH as usize],
            line_buffer_attr: [0; SCREEN_WIDTH as usize],
            frame_buffer: Ref::clone(frame_buffer),
            vram: Ref::clone(vram),
            oam: Ref::clone(oam),
            oam_lock: Ref::clone(oam_lock),
            interrupt_flag: Ref::clone(interrupt_flag),
            dmg_palette: dmg_palette.clone(),
        }
    }

    pub fn set_dmg_palette(&mut self, palette: &[Color; 4]) {
        self.dmg_palette = palette.clone();
    }

    pub fn tick(&mut self) {
        self.lx += 1;
        if self.lx == DOTS_PER_LINE {
            self.lx = 0;

            self.ly += 1;
            if self.ly == LINES_PER_FRAME as u8 {
                self.ly = 0;
                self.frame += 1;
                self.window_rendering_counter = 0;
            }
        }

        if !self.ppu_enable {
            self.mode = MODE_HBLANK;
            self.prev_lcd_interrupt = false;
            return;
        }

        if VISIBLE_RANGE.contains(&(self.ly as u64)) {
            if self.lx < 80 {
                self.set_mode(MODE_OAM_SEARCH);
            } else {
                // FIXME: Calculate accurate timing
                let transfer_period = 172 + self.scroll_x as u64 % 8;

                if self.lx < 80 + transfer_period {
                    self.set_mode(MODE_TRANSFER);
                } else {
                    self.set_mode(MODE_HBLANK);
                }
            }
        } else {
            self.set_mode(MODE_VBLANK);
        }

        self.update_lcd_interrupt();
    }

    fn set_mode(&mut self, mode: u8) {
        if self.mode != mode {
            if mode == MODE_VBLANK {
                *self.interrupt_flag.borrow_mut() |= INT_VBLANK;
            }
            if mode == MODE_TRANSFER {
                self.render_line();
            }
            *self.oam_lock.borrow_mut() = matches!(mode, MODE_OAM_SEARCH | MODE_TRANSFER);
        }
        self.mode = mode;
    }

    fn update_lcd_interrupt(&mut self) {
        let cur_lcd_interrupt = match self.mode {
            MODE_HBLANK => self.hblank_interrupt_enable,
            MODE_VBLANK => {
                self.vblank_interrupt_enable
                    || (self.ly as u64 == VISIBLE_RANGE.end
                        && self.lx < 80
                        && self.oam_interrupt_enable)
            }
            MODE_OAM_SEARCH => self.oam_interrupt_enable,
            _ => false,
        } || (self.lyc_interrupt_enable && self.ly == self.lyc);

        if !self.prev_lcd_interrupt && cur_lcd_interrupt {
            *self.interrupt_flag.borrow_mut() |= INT_LCD_STAT;
        }
        self.prev_lcd_interrupt = cur_lcd_interrupt;
    }

    pub fn frame(&self) -> u64 {
        self.frame
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        let data = match addr & 0xff {
            // LCDC: LCD Control (R/W)
            0x40 => pack! {
                7 => self.ppu_enable,
                6 => self.window_tile_map_select,
                5 => self.window_enable,
                4 => self.bg_and_window_tile_data_select,
                3 => self.bg_tile_map_select,
                2 => self.obj_size,
                1 => self.obj_enable,
                0 => self.bg_and_window_enable,
            },
            // STAT: LCDC Status (R/W)
            0x41 => pack! {
                7     => true,
                6     => self.lyc_interrupt_enable,
                5     => self.oam_interrupt_enable,
                4     => self.vblank_interrupt_enable,
                3     => self.hblank_interrupt_enable,
                2     => self.lyc == self.ly,
                0..=1 => self.mode,
            },
            // SCY: Scroll Y (R/W)
            0x42 => self.scroll_y,
            // SCX: Scroll X (R/W)
            0x43 => self.scroll_x,
            // LY: LCDC Y-Coordinate (R)
            0x44 => self.ly,
            // LYC: LY Compare (R/W)
            0x45 => self.lyc,
            // BGP: BG Palette Data (R/W)
            0x47 => pack! {
                6..=7 => self.bg_palette[3],
                4..=5 => self.bg_palette[2],
                2..=3 => self.bg_palette[1],
                0..=1 => self.bg_palette[0],
            },
            // OBP0/1: Object Palette 0/1 Data (R/W)
            0x48 | 0x49 => {
                let ix = (addr & 0x1) as usize;
                pack! {
                    6..=7 => self.obj_palette[ix][3],
                    4..=5 => self.obj_palette[ix][2],
                    2..=3 => self.obj_palette[ix][1],
                    0..=1 => self.obj_palette[ix][0],
                }
            }
            // WY: Window Y Position (R/W)
            0x4a => self.window_y,
            // WX: Window X Position (R/W)
            0x4b => self.window_x,
            _ => todo!("Read from LCD I/O: ${addr:04X}"),
        };
        // trace!("PPU Read: ${addr:04X} = ${data:02X}");
        data
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        trace!("PPU Write: ${addr:04X} = ${data:02X}");
        match addr & 0xff {
            // LCDC: LCD Control (R/W)
            0x40 => {
                let v = data.view_bits::<Lsb0>();
                if self.ppu_enable && !v[7] && self.mode != MODE_VBLANK {
                    error!("Disabling the display outside of the VBlank period");
                }
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
            0x42 => {
                if self.mode == MODE_TRANSFER {
                    log::info!(
                        "SCY changed in mode3: SCY={data:3} FRM:{} Y:{:3} X:{:3}",
                        self.frame,
                        self.ly,
                        self.lx
                    );
                }
                self.scroll_y = data
            }
            // SCX: Scroll X (R/W)
            0x43 => {
                if self.mode == MODE_TRANSFER {
                    log::info!(
                        "SCX changed in mode3: SCX={data:3} FRM:{} Y:{:3} X:{:3}",
                        self.frame,
                        self.ly,
                        self.lx
                    );
                }
                self.scroll_x = data
            }
            // LYC: LY Compare (R/W)
            0x45 => self.lyc = data,
            // BGP: BG Palette Data (R/W)
            0x47 => {
                let v = data.view_bits::<Lsb0>();
                self.bg_palette[3] = v[6..=7].load();
                self.bg_palette[2] = v[4..=5].load();
                self.bg_palette[1] = v[2..=3].load();
                self.bg_palette[0] = v[0..=1].load();
            }
            // OBP0/1: Object Palette 0/1 Data (R/W)
            0x48 | 0x49 => {
                let ix = (addr & 0x1) as usize;
                let v = data.view_bits::<Lsb0>();
                self.obj_palette[ix][3] = v[6..=7].load();
                self.obj_palette[ix][2] = v[4..=5].load();
                self.obj_palette[ix][1] = v[2..=3].load();
                self.obj_palette[ix][0] = v[0..=1].load();
            }
            // WY: Window Y Position (R/W)
            0x4a => self.window_y = data,
            // WX: Window X Position (R/W)
            0x4b => {
                self.window_x = data;
                // WX values 0 and 166 are unreliable due to hardware bugs.
                // If WX is set to 0, the window will “stutter” horizontally when SCX changes (depending on SCX % 8).
                // If WX is set to 166, the window will span the entirety of the following scanline.
                if self.window_x == 0 || self.window_x == 166 {
                    warn!("WX value 0 or 166 is unreliable");
                }
            }
            _ => warn!("Unusable write to I/O: ${addr:04X} = ${data:02X}"),
        }
    }
}

impl Ppu {
    fn render_line(&mut self) {
        self.line_buffer.fill(0);
        self.line_buffer_attr.fill(ATTR_NONE);
        if self.ppu_enable && self.bg_and_window_enable {
            self.render_bg_line();
        }
        if self.ppu_enable && self.obj_enable {
            self.render_obj_line();
        }
        let mut fb = self.frame_buffer.borrow_mut();
        for x in 0..SCREEN_WIDTH as usize {
            let c = self.line_buffer[x];
            fb.set(x, self.ly as _, self.dmg_palette[(c & 3) as usize])
        }
    }

    fn render_bg_line(&mut self) {
        let vram = self.vram.borrow();
        let tile_data = if self.bg_and_window_tile_data_select {
            0x0000
        } else {
            0x1000
        };
        let bg_tile_map = if self.bg_tile_map_select {
            0x1c00
        } else {
            0x1800
        };
        let window_tile_map = if self.window_tile_map_select {
            0x1c00
        } else {
            0x1800
        };

        let y = self.ly.wrapping_add(self.scroll_y);
        let is_in_window_y_range = self.ly >= self.window_y;
        let mut window_rendered = false;

        for scr_x in 0..SCREEN_WIDTH as u8 {
            let is_in_window_x_range = scr_x + 7 >= self.window_x;

            let (x, y, tile_map) =
                if !(self.window_enable && is_in_window_y_range && is_in_window_x_range) {
                    (scr_x.wrapping_add(self.scroll_x), y, bg_tile_map)
                } else {
                    window_rendered = true;
                    (
                        scr_x - self.window_x + 7,
                        self.window_rendering_counter,
                        window_tile_map,
                    )
                };

            let tile_x = x as usize / 8;
            let tile_y = y as usize / 8;
            let ofs_x = x as usize % 8;
            let ofs_y = y as usize % 8;

            let tile_ix = vram[tile_map + tile_y * 32 + tile_x];

            let mut tile_addr = tile_data + (tile_ix as usize * 16);
            if tile_addr >= 0x1800 {
                tile_addr -= 0x1000;
            }

            let lo = vram[tile_addr + ofs_y * 2];
            let hi = vram[tile_addr + ofs_y * 2 + 1];

            let b = (lo >> (7 - ofs_x)) & 1 | ((hi >> (7 - ofs_x)) & 1) << 1;

            self.line_buffer[scr_x as usize] = self.bg_palette[b as usize];
            self.line_buffer_attr[scr_x as usize] = if b != 0 { ATTR_BG } else { ATTR_NONE };
        }

        if window_rendered {
            self.window_rendering_counter += 1;
        }
    }

    fn render_obj_line(&mut self) {
        let oam = self.oam.borrow();
        let vram = self.vram.borrow();
        let w = self.line_buffer.len();

        let obj_size = if self.obj_size { 16 } else { 8 };

        let mut obj_count = 0;
        let mut render_objs = [(0xff, 0xff); 10];

        for i in 0..40 {
            let r = &oam[i * 4..i * 4 + 4];
            let y = r[0];
            let x = r[1];
            if (y..y + obj_size).contains(&(self.ly + 16)) {
                render_objs[obj_count] = (x, i);
                obj_count += 1;
                if obj_count >= 10 {
                    break;
                }
            }
        }

        // FIXME:
        // if !cgb_mode {
        render_objs[0..obj_count].sort();
        // }

        for i in 0..obj_count {
            let i = render_objs[i].1;
            let r = &oam[i * 4..i * 4 + 4];

            let y = r[0];
            let x = r[1];
            let tile_index = r[2];

            let v = r[3].view_bits::<Lsb0>();
            let bg_and_window_over_obj = v[7];
            let y_flip = v[6];
            let x_flip = v[5];

            // Non CGB Mode Only
            let palette_number = v[4] as usize;

            // CGB Mode Only
            // let tile_vram_bank = v[3];
            // let palette_number = v[0..=2].load();

            let ofs_y = self.ly + 16 - y;

            let tile_addr = if obj_size == 8 {
                let ofs_y = if y_flip { 7 - ofs_y } else { ofs_y };
                (tile_index as usize * 16) + ofs_y as usize * 2
            } else {
                let ofs_y = if y_flip { 15 - ofs_y } else { ofs_y };
                ((tile_index & !1) as usize * 16 + if ofs_y >= 8 { 16 } else { 0 })
                    + (ofs_y & 7) as usize * 2
            };

            let lo = vram[tile_addr];
            let hi = vram[tile_addr + 1];

            for ofs_x in 0..8 {
                let scr_x = x as usize + ofs_x;
                if !(8..w + 8).contains(&scr_x) {
                    continue;
                }
                let scr_x = scr_x - 8;
                let ofs_x = if x_flip { 7 - ofs_x } else { ofs_x };

                let b = (lo >> (7 - ofs_x)) & 1 | ((hi >> (7 - ofs_x)) & 1) << 1;

                if b != 0 {
                    let c = self.obj_palette[palette_number][b as usize];
                    match self.line_buffer_attr[scr_x] {
                        ATTR_NONE => self.line_buffer[scr_x] = c,
                        ATTR_BG => {
                            if !bg_and_window_over_obj {
                                self.line_buffer[scr_x] = c;
                            }
                        }
                        _ => {}
                    }
                    self.line_buffer_attr[scr_x] = ATTR_OBJ;
                }
            }
        }
    }
}
