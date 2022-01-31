use crate::{
    bus::Bus,
    cpu::Cpu,
    io::Io,
    ppu::Ppu,
    rom::{CgbFlag, Rom},
    sound::Sound,
    util::Ref,
};

pub struct GameBoy {
    cpu: Cpu,
    rom: Ref<Rom>,
}

impl GameBoy {
    pub fn new(rom: Rom, force_dmg: bool) -> Self {
        let rom = Ref::new(rom);
        let ppu = Ref::new(Ppu::new());
        let sound = Ref::new(Sound::new());
        let io = Ref::new(Io::new(&ppu, &sound));
        let bus = Ref::new(Bus::new(&rom, &io));
        let mut cpu = Cpu::new(&bus);

        let is_gbc = match rom.borrow().cgb_flag {
            CgbFlag::NonCgb => false,
            CgbFlag::SupportCgb => !force_dmg,
            CgbFlag::OnlyCgb => true,
        };

        // Set up the contents of registers after internal ROM execution
        if !is_gbc {
            cpu.reg.a = 0x01;
            cpu.reg.f.unpack(0xB0);
            cpu.reg.b = 0x00;
            cpu.reg.c = 0x13;
            cpu.reg.d = 0x00;
            cpu.reg.e = 0xD8;
            cpu.reg.h = 0x01;
            cpu.reg.l = 0x4D;
            cpu.reg.sp = 0xFFFE;
            cpu.reg.pc = 0x0100;
        } else {
            cpu.reg.a = 0x11;
            cpu.reg.f.unpack(0x80);
            cpu.reg.b = 0x00;
            cpu.reg.c = 0x00;
            cpu.reg.d = 0xFF;
            cpu.reg.e = 0x56;
            cpu.reg.h = 0x00;
            cpu.reg.l = 0x0D;
            cpu.reg.sp = 0xFFFE;
            cpu.reg.pc = 0x0100;
        }

        Self { cpu, rom }
    }

    pub fn exec_frame(&mut self) {
        self.cpu.tick();
    }
}
