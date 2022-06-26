use bitvec::prelude::*;
use log::{debug, error, trace, warn};
use serde::{Deserialize, Serialize};

use crate::{
    consts::{
        DOTS_PER_LINE, INT_LCD_STAT, INT_VBLANK, LINES_PER_FRAME, SCREEN_WIDTH, VISIBLE_RANGE,
    },
    context,
    interface::{Color, FrameBuffer},
    util::{pack, trait_alias},
};

trait_alias!(pub trait Context = context::Vram + context::Oam + context::InterruptFlag + context::Model);

#[derive(Default, Serialize, Deserialize)]
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
    mode: Mode,
    prev_lcd_interrupt: bool,

    scroll_y: u8,
    scroll_x: u8,
    window_y: u8,
    window_x: u8,

    bg_pal: [u8; 4],
    obj_pal: [[u8; 4]; 2],

    bg_col_pal_incr: bool,
    bg_col_pal_addr: u8,
    #[serde(with = "serde_bytes")]
    bg_col_pal: Vec<u8>,
    obj_col_pal_incr: bool,
    obj_col_pal_addr: u8,
    #[serde(with = "serde_bytes")]
    obj_col_pal: Vec<u8>,

    lyc: u8,
    ly: u8,
    lx: u64,
    frame: u64,
    window_rendering_counter: u8,

    dmg_palette: [Color; 4],

    #[serde(with = "serde_bytes")]
    line_buffer: Vec<u8>,
    // #[serde(with = "serde_bytes")]
    line_buffer_col: Vec<u16>,
    #[serde(with = "serde_bytes")]
    line_buffer_attr: Vec<u8>,

    #[serde(skip)]
    render_graphics: bool,

    #[serde(skip)]
    frame_buffer: FrameBuffer,
}

#[derive(PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
#[repr(u8)]
pub enum Mode {
    Hblank = 0,
    Vblank = 1,
    OamSearch = 2,
    Transfer = 3,
}

impl Default for Mode {
    fn default() -> Self {
        Mode::Hblank
    }
}

impl Ppu {
    pub fn new(dmg_palette: &[Color; 4]) -> Self {
        Self {
            bg_col_pal: vec![0; 64],
            obj_col_pal: vec![0; 64],
            line_buffer: vec![0; SCREEN_WIDTH as usize],
            line_buffer_col: vec![0; SCREEN_WIDTH as usize],
            line_buffer_attr: vec![0; SCREEN_WIDTH as usize],
            dmg_palette: *dmg_palette,
            ..Default::default()
        }
    }

    pub fn dmg_palette(&self) -> &[Color; 4] {
        &self.dmg_palette
    }

    pub fn set_dmg_palette(&mut self, palette: &[Color; 4]) {
        self.dmg_palette = *palette;
    }

    pub fn set_render_graphics(&mut self, render_graphics: bool) {
        self.render_graphics = render_graphics;
    }

    pub fn frame_buffer(&self) -> &FrameBuffer {
        &self.frame_buffer
    }

    pub fn tick(&mut self, ctx: &mut impl Context) {
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
            self.mode = Mode::Hblank;
            self.prev_lcd_interrupt = false;
            return;
        }

        if VISIBLE_RANGE.contains(&(self.ly as u64)) {
            if self.lx < 80 {
                self.set_mode(ctx, Mode::OamSearch);
            } else {
                // FIXME: Calculate accurate timing
                let transfer_period = 172 + self.scroll_x as u64 % 8;

                if self.lx < 80 + transfer_period {
                    self.set_mode(ctx, Mode::Transfer);
                } else {
                    self.set_mode(ctx, Mode::Hblank);
                }
            }
        } else {
            self.set_mode(ctx, Mode::Vblank);
        }

        self.update_lcd_interrupt(ctx);
    }

    fn set_mode(&mut self, ctx: &mut impl Context, mode: Mode) {
        if self.mode != mode {
            if mode == Mode::Vblank {
                ctx.set_interrupt_flag_bit(INT_VBLANK);
            }
            if mode == Mode::Transfer {
                self.render_line(ctx);
            }
            // ctx.set_vram_lock(self.vram_locked());
            ctx.set_oam_lock(self.oam_locked());
        }
        self.mode = mode;
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    fn update_lcd_interrupt(&mut self, ctx: &mut impl Context) {
        let cur_lcd_interrupt = match self.mode {
            Mode::Hblank => self.hblank_interrupt_enable,
            Mode::Vblank => {
                self.vblank_interrupt_enable
                    || (self.ly as u64 == VISIBLE_RANGE.end
                        && self.lx < 80
                        && self.oam_interrupt_enable)
            }
            Mode::OamSearch => self.oam_interrupt_enable,
            _ => false,
        } || (self.lyc_interrupt_enable && self.ly == self.lyc);

        if !self.prev_lcd_interrupt && cur_lcd_interrupt {
            ctx.set_interrupt_flag_bit(INT_LCD_STAT);
        }
        self.prev_lcd_interrupt = cur_lcd_interrupt;
    }

    pub fn frame(&self) -> u64 {
        self.frame
    }

    pub fn read(&mut self, ctx: &impl Context, addr: u16) -> u8 {
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
                0..=1 => self.mode as u8,
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
                6..=7 => self.bg_pal[3],
                4..=5 => self.bg_pal[2],
                2..=3 => self.bg_pal[1],
                0..=1 => self.bg_pal[0],
            },
            // OBP0/1: Object Palette 0/1 Data (R/W)
            0x48 | 0x49 => {
                let ix = (addr & 0x1) as usize;
                pack! {
                    6..=7 => self.obj_pal[ix][3],
                    4..=5 => self.obj_pal[ix][2],
                    2..=3 => self.obj_pal[ix][1],
                    0..=1 => self.obj_pal[ix][0],
                }
            }
            // WY: Window Y Position (R/W)
            0x4A => self.window_y,
            // WX: Window X Position (R/W)
            0x4B => self.window_x,

            // BCPS/BGPI: (Background Color Palette Specification or Background Palette Index) - CGB Mode Only
            0x68 => {
                if ctx.running_mode().is_cgb() {
                    pack! {
                        7 => self.bg_col_pal_incr,
                        6 => true,
                        0..=5 => self.bg_col_pal_addr,
                    }
                } else {
                    !0
                }
            }
            // BCPD/BGPD: (Background Color Palette Data) - CGB Mode Only
            0x69 => {
                if ctx.running_mode().is_cgb() {
                    self.bg_col_pal[self.bg_col_pal_addr as usize]
                } else {
                    !0
                }
            }
            // OCPS/OBPI: (OBJ Color Palette Specification or OBJ Palette Index) - CGB Mode Only
            0x6A => {
                if ctx.running_mode().is_cgb() {
                    pack! {
                        7 => self.obj_col_pal_incr,
                        6 => true,
                        0..=5 => self.obj_col_pal_addr,
                    }
                } else {
                    !0
                }
            }
            // OCPD/OBPD: (OBJ Color Palette Data) - CGB Mode Only
            0x6B => {
                if ctx.running_mode().is_cgb() {
                    self.obj_col_pal[self.obj_col_pal_addr as usize]
                } else {
                    !0
                }
            }

            // OPRI - Object Priority Mode - CGB Mode Only
            0x6C => !0,

            _ => todo!("Read from LCD I/O: ${addr:04X}"),
        };
        // trace!("PPU Read: ${addr:04X} = ${data:02X}");
        data
    }

    pub fn write(&mut self, ctx: &impl Context, addr: u16, data: u8) {
        trace!("PPU Write: ${addr:04X} = ${data:02X}");
        match addr & 0xff {
            // LCDC: LCD Control (R/W)
            0x40 => {
                let v = data.view_bits::<Lsb0>();
                if self.ppu_enable && !v[7] && self.mode != Mode::Vblank {
                    error!("Disabling the display outside of the VBlank period");
                }

                if !self.ppu_enable && v[7] {
                    self.ly = 0;
                    self.lx = 0;
                    self.frame += 1;
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
                if self.scroll_y != data && self.mode == Mode::Transfer {
                    debug!(
                        "SCY changed in mode3: SCY={data:3} FRM:{} Y:{:3} X:{:3}",
                        self.frame, self.ly, self.lx
                    );
                }
                self.scroll_y = data
            }
            // SCX: Scroll X (R/W)
            0x43 => {
                if self.scroll_x != data && self.mode == Mode::Transfer {
                    debug!(
                        "SCX changed in mode3: SCX={data:3} FRM:{} Y:{:3} X:{:3}",
                        self.frame, self.ly, self.lx
                    );
                }
                self.scroll_x = data
            }
            // LYC: LY Compare (R/W)
            0x45 => self.lyc = data,
            // BGP: BG Palette Data (R/W)
            0x47 => {
                let v = data.view_bits::<Lsb0>();
                self.bg_pal[3] = v[6..=7].load();
                self.bg_pal[2] = v[4..=5].load();
                self.bg_pal[1] = v[2..=3].load();
                self.bg_pal[0] = v[0..=1].load();
            }
            // OBP0/1: Object Palette 0/1 Data (R/W)
            0x48 | 0x49 => {
                let ix = (addr & 0x1) as usize;
                let v = data.view_bits::<Lsb0>();
                self.obj_pal[ix][3] = v[6..=7].load();
                self.obj_pal[ix][2] = v[4..=5].load();
                self.obj_pal[ix][1] = v[2..=3].load();
                self.obj_pal[ix][0] = v[0..=1].load();
            }
            // WY: Window Y Position (R/W)
            0x4A => self.window_y = data,
            // WX: Window X Position (R/W)
            0x4B => {
                self.window_x = data;
                // WX values 0 and 166 are unreliable due to hardware bugs.
                // If WX is set to 0, the window will “stutter” horizontally when SCX changes (depending on SCX % 8).
                // If WX is set to 166, the window will span the entirety of the following scanline.

                // if self.window_x == 0 || self.window_x == 166 {
                //     warn!("WX value 0 or 166 is unreliable: WX = {}", self.window_x);
                // }
            }

            // BCPS/BGPI: (Background Color Palette Specification or Background Palette Index) - CGB Mode Only
            0x68 => {
                if ctx.model().is_cgb() {
                    let v = data.view_bits::<Lsb0>();
                    self.bg_col_pal_incr = v[7];
                    self.bg_col_pal_addr = v[0..=5].load();
                } else {
                    warn!("Writing to BCPS/BGPI in DMG mode");
                }
            }
            // BCPD/BGPD: (Background Color Palette Specification or Background Palette Index) - CGB Mode Only
            0x69 => {
                if ctx.model().is_cgb() {
                    self.bg_col_pal[self.bg_col_pal_addr as usize] = data;
                    if self.bg_col_pal_incr {
                        self.bg_col_pal_addr = (self.bg_col_pal_addr + 1) & 0x3f;
                    }
                } else {
                    warn!("Writing to BCPD/BGPD in DMG mode");
                }
            }
            // OCPS/OBPI: (OBJ Color Palette Specification or OBJ Palette Index) - CGB Mode Only
            0x6A => {
                if ctx.model().is_cgb() {
                    let v = data.view_bits::<Lsb0>();
                    self.obj_col_pal_incr = v[7];
                    self.obj_col_pal_addr = v[0..=5].load();
                } else {
                    warn!("Writing to OCPS/OBPI in DMG mode");
                }
            }
            // OCPD/OBPD: (OBJ Color Palette Specification or OBJ Palette Index) - CGB Mode Only
            0x6B => {
                if ctx.model().is_cgb() {
                    self.obj_col_pal[self.obj_col_pal_addr as usize] = data;
                    if self.obj_col_pal_incr {
                        self.obj_col_pal_addr = (self.obj_col_pal_addr + 1) & 0x3f;
                    }
                } else {
                    warn!("Writing to OCPD/OBPD in DMG mode");
                }
            }

            // OPRI - CGB Mode Only - Object Priority Mode
            0x6C => {
                if ctx.model().is_cgb() {
                    // ???
                    debug!("ORPI = ${data:02X}");
                } else {
                    warn!("OPRI write in DMG mode");
                }
            }
            _ => warn!("Unusable write to I/O: ${addr:04X} = ${data:02X}"),
        }
    }

    // fn vram_locked(&self) -> bool {
    //     // self.mode == MODE_TRANSFER
    //     false
    // }

    fn oam_locked(&self) -> bool {
        self.mode == Mode::OamSearch || self.mode == Mode::Transfer
    }
}

fn decode_color(c: u16) -> Color {
    let v = c.view_bits::<Lsb0>();
    let r = v[0..=4].load::<u8>();
    let g = v[5..=9].load::<u8>();
    let b = v[10..=14].load::<u8>();
    Color {
        r: r << 3 | r >> 2,
        g: g << 3 | g >> 2,
        b: b << 3 | b >> 2,
    }
}

const Z_ORD_OBJ_HIGHEST: u8 = 10;
const Z_ORD_BG_HIGH: u8 = 8;
const Z_ORD_OBJ_HIGH: u8 = 6;
const Z_ORD_BG_LOW: u8 = 4;
const Z_ORD_OBJ_LOW: u8 = 2;
const Z_ORD_NULL: u8 = 0;

impl Ppu {
    fn render_line(&mut self, ctx: &impl Context) {
        if !self.render_graphics {
            return;
        }

        self.line_buffer.fill(0);
        self.line_buffer_col.fill(0);
        self.line_buffer_attr.fill(Z_ORD_NULL);
        self.render_bg_line(ctx);
        self.render_obj_line(ctx);

        let y = self.ly as usize;
        if ctx.model().is_cgb() {
            for x in 0..SCREEN_WIDTH as usize {
                self.frame_buffer
                    .set(x, y, decode_color(self.line_buffer_col[x]));
            }
        } else {
            for x in 0..SCREEN_WIDTH as usize {
                let c = self.line_buffer[x];
                let color = self.dmg_palette[(c & 3) as usize];
                self.frame_buffer.set(x, y, color)
            }
        }
    }

    fn render_bg_line(&mut self, ctx: &impl Context) {
        let is_cgb = ctx.model().is_cgb();
        let is_cgb_mode = ctx.running_mode() == context::RunningMode::Cgb;

        if !(self.ppu_enable && (self.bg_and_window_enable || is_cgb_mode)) {
            return;
        }

        let tile_data: u16 = if self.bg_and_window_tile_data_select {
            0x0000
        } else {
            0x1000
        };
        let bg_tile_map: u16 = if self.bg_tile_map_select {
            0x1C00
        } else {
            0x1800
        };
        let window_tile_map: u16 = if self.window_tile_map_select {
            0x1C00
        } else {
            0x1800
        };

        let y = self.ly.wrapping_add(self.scroll_y);
        let is_in_window_y_range = self.ly >= self.window_y;
        let mut window_rendered = false;

        let vram = ctx.vram();

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

            let tile_x = x as u16 / 8;
            let tile_y = y as u16 / 8;
            let ofs_x = x as u16 % 8;
            let ofs_y = y as u16 % 8;

            let tile_ix_addr = (tile_map + tile_y * 32 + tile_x) as usize;
            let tile_ix = vram[tile_ix_addr];

            let tile_attr = if is_cgb {
                vram[0x2000 | tile_ix_addr]
            } else {
                0
            };

            let mut tile_addr = tile_data + (tile_ix as u16 * 16);
            if tile_addr >= 0x1800 {
                tile_addr -= 0x1000;
            }

            let tile_attr = tile_attr.view_bits::<Lsb0>();
            let bg_to_oam_priority = tile_attr[7];
            let vertical_flip = tile_attr[6];
            let horizontal_flip = tile_attr[5];
            let vram_bank = if tile_attr[3] { 0x2000 } else { 0 };
            let pal_num: u8 = tile_attr[0..=2].load();

            let ofs_x = if horizontal_flip { 7 - ofs_x } else { ofs_x };
            let ofs_y = if vertical_flip { 7 - ofs_y } else { ofs_y };

            let lo = vram[(vram_bank + tile_addr + ofs_y * 2) as usize];
            let hi = vram[(vram_bank + tile_addr + ofs_y * 2 + 1) as usize];

            let b = (lo >> (7 - ofs_x)) & 1 | ((hi >> (7 - ofs_x)) & 1) << 1;

            if !is_cgb {
                self.line_buffer[scr_x as usize] = self.bg_pal[b as usize];
            } else if !is_cgb_mode {
                let b = self.bg_pal[b as usize] as usize * 2;
                self.line_buffer_col[scr_x as usize] =
                    u16::from_le_bytes(self.bg_col_pal[b..b + 2].try_into().unwrap());
            } else {
                let pal_ix = (pal_num * 4 + b) as usize * 2;
                self.line_buffer_col[scr_x as usize] =
                    u16::from_le_bytes(self.bg_col_pal[pal_ix..pal_ix + 2].try_into().unwrap());
            }
            if b != 0 {
                self.line_buffer_attr[scr_x as usize] = if bg_to_oam_priority {
                    Z_ORD_BG_HIGH
                } else {
                    Z_ORD_BG_LOW
                };
            }
        }

        if window_rendered {
            self.window_rendering_counter += 1;
        }
    }

    fn render_obj_line(&mut self, ctx: &impl Context) {
        if !(self.ppu_enable && self.obj_enable) {
            return;
        }

        let is_cgb = ctx.model().is_cgb();
        let is_cgb_mode = ctx.running_mode() == context::RunningMode::Cgb;

        let w = self.line_buffer.len();

        let obj_size = if self.obj_size { 16 } else { 8 };

        let mut obj_count = 0;
        let mut render_objs = [(0xff, 0xff); 10];

        let oam = ctx.oam();
        let vram = ctx.vram();

        for i in 0..40 {
            let r = &oam[i * 4..(i + 1) * 4];
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

        if !is_cgb_mode {
            render_objs[0..obj_count].sort_unstable();
        }

        for &(_, i) in &render_objs[0..obj_count] {
            let r = &oam[i * 4..(i + 1) * 4];
            let y = r[0];
            let x = r[1];
            let tile_index = r[2];
            let v = r[3].view_bits::<Lsb0>();
            let bg_and_window_over_obj = v[7];
            let y_flip = v[6];
            let x_flip = v[5];

            let (palette_number, tile_vram_bank) = if !is_cgb_mode {
                (v[4] as u8, 0)
            } else {
                (v[0..=2].load(), if v[3] { 0x2000 } else { 0 })
            };

            let ofs_y = self.ly + 16 - y;

            let tile_addr = if obj_size == 8 {
                let ofs_y = if y_flip { 7 - ofs_y } else { ofs_y };
                (tile_index as u16 * 16) + ofs_y as u16 * 2
            } else {
                let ofs_y = if y_flip { 15 - ofs_y } else { ofs_y };
                ((tile_index & !1) as u16 * 16 + if ofs_y >= 8 { 16 } else { 0 })
                    + (ofs_y & 7) as u16 * 2
            };

            let lo = vram[(tile_vram_bank | tile_addr) as usize];
            let hi = vram[(tile_vram_bank | (tile_addr + 1)) as usize];

            let z = if is_cgb_mode && !self.bg_and_window_enable {
                Z_ORD_OBJ_HIGHEST
            } else if bg_and_window_over_obj {
                Z_ORD_OBJ_LOW
            } else {
                Z_ORD_OBJ_HIGH
            };

            for ofs_x in 0..8 {
                let scr_x = x as usize + ofs_x;
                if !(8..w + 8).contains(&scr_x) {
                    continue;
                }
                let scr_x = scr_x - 8;
                let ofs_x = if x_flip { 7 - ofs_x } else { ofs_x };

                let b = (lo >> (7 - ofs_x)) & 1 | ((hi >> (7 - ofs_x)) & 1) << 1;

                if b != 0 {
                    if self.line_buffer_attr[scr_x] & 1 == 0 && z > self.line_buffer_attr[scr_x] {
                        if !is_cgb {
                            self.line_buffer[scr_x] =
                                self.obj_pal[palette_number as usize][b as usize];
                        } else if !is_cgb_mode {
                            let b = (palette_number * 4
                                + self.obj_pal[palette_number as usize][b as usize])
                                as usize
                                * 2;
                            self.line_buffer_col[scr_x] =
                                u16::from_le_bytes(self.obj_col_pal[b..b + 2].try_into().unwrap());
                        } else {
                            let pal_ix = (palette_number * 4 + b) as usize * 2;
                            self.line_buffer_col[scr_x] = u16::from_le_bytes(
                                self.obj_col_pal[pal_ix..pal_ix + 2].try_into().unwrap(),
                            );
                        }
                    }
                    self.line_buffer_attr[scr_x] |= 1;
                }
            }
        }
    }
}
