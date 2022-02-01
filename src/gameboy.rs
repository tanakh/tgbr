use crate::{
    apu::Apu,
    bus::Bus,
    consts::{SCREEN_HEIGHT, SCREEN_WIDTH},
    cpu::Cpu,
    interface::FrameBuffer,
    io::Io,
    mbc::create_mbc,
    ppu::Ppu,
    rom::{CgbFlag, Rom},
    util::Ref,
};

pub struct GameBoy {
    cpu: Cpu,
    ppu: Ref<Ppu>,
    rom: Ref<Rom>,
    frame_buffer: Ref<FrameBuffer>,
}

impl GameBoy {
    pub fn new(rom: Rom, force_dmg: bool) -> Self {
        let rom = Ref::new(rom);
        let mbc = create_mbc(&rom);
        let frame_buffer = Ref::new(FrameBuffer::new(
            SCREEN_WIDTH as usize,
            SCREEN_HEIGHT as usize,
        ));

        let interrupt_enable = Ref::new(0x00);
        let interrupt_flag = Ref::new(0x00);

        let vram = Ref::new(vec![0; 0x2000]);
        let oam = Ref::new(vec![0; 0xA0]);

        let ppu = Ref::new(Ppu::new(&vram, &oam, &interrupt_flag, &frame_buffer));
        let apu = Ref::new(Apu::new());

        let io = Ref::new(Io::new(&ppu, &apu, &interrupt_enable, &interrupt_flag));

        let bus = Ref::new(Bus::new(&mbc, &vram, &oam, &io));
        let mut cpu = Cpu::new(&bus, &interrupt_enable, &interrupt_flag);

        let is_gbc = match rom.borrow().cgb_flag {
            CgbFlag::NonCgb => false,
            CgbFlag::SupportCgb => !force_dmg,
            CgbFlag::OnlyCgb => true,
        };

        // Set up the contents of registers after internal ROM execution
        let reg = cpu.register();
        if !is_gbc {
            reg.a = 0x01;
            reg.f.unpack(0xB0);
            reg.b = 0x00;
            reg.c = 0x13;
            reg.d = 0x00;
            reg.e = 0xD8;
            reg.h = 0x01;
            reg.l = 0x4D;
            reg.sp = 0xFFFE;
            reg.pc = 0x0100;
        } else {
            reg.a = 0x11;
            reg.f.unpack(0x80);
            reg.b = 0x00;
            reg.c = 0x00;
            reg.d = 0xFF;
            reg.e = 0x56;
            reg.h = 0x00;
            reg.l = 0x0D;
            reg.sp = 0xFFFE;
            reg.pc = 0x0100;
        }

        Self {
            cpu,
            ppu,
            rom,
            frame_buffer,
        }
    }

    pub fn exec_frame(&mut self) {
        let start_frame = self.ppu.borrow().frame();

        while start_frame == self.ppu.borrow().frame() {
            self.cpu.tick();
            for _ in 0..4 {
                self.ppu.borrow_mut().tick();
            }
        }
    }

    pub fn frame_buffer(&self) -> &Ref<FrameBuffer> {
        &self.frame_buffer
    }
}
