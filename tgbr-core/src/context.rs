use ambassador::{delegatable_trait, Delegate};
use serde::{Deserialize, Serialize};

#[delegatable_trait]
pub trait Bus {
    fn tick(&mut self);
    fn read(&mut self, addr: u16) -> u8;
    fn read_immutable(&mut self, addr: u16) -> Option<u8>;
    fn write(&mut self, addr: u16, data: u8);
}

#[delegatable_trait]
pub trait InterruptFlag {
    fn interrupt_enable(&mut self) -> u8;
    fn set_interrupt_enable(&mut self, data: u8);
    fn interrupt_flag(&mut self) -> u8;
    fn set_interrupt_flag(&mut self, data: u8);

    fn set_interrupt_flag_bit(&mut self, bit: usize) {
        let new_flag = self.interrupt_flag() | (1 << bit);
        self.set_interrupt_flag(new_flag);
    }
    fn clear_interrupt_flag_bit(&mut self, bit: usize) {
        let new_flag = self.interrupt_flag() & !(1 << bit);
        self.set_interrupt_flag(new_flag);
    }
}

#[delegatable_trait]
pub trait Vram {
    fn read_vram(&self, addr: u16, force: bool) -> u8;
    fn write_vram(&mut self, addr: u16, data: u8, force: bool);
    fn lock_vram(&mut self, lock: bool);
}

#[delegatable_trait]
pub trait Oam {
    fn read_oam(&self, addr: u8, force: bool) -> u8;
    fn write_oam(&mut self, addr: u8, data: u8, force: bool);
    fn lock_oam(&mut self, lock: bool);
}

#[delegatable_trait]
pub trait Ppu {
    fn ppu(&self) -> &crate::ppu::Ppu;
    fn ppu_mut(&mut self) -> &mut crate::ppu::Ppu;
}

#[delegatable_trait]
pub trait Apu {
    fn apu(&self) -> &crate::apu::Apu;
    fn apu_mut(&mut self) -> &mut crate::apu::Apu;
}

pub trait Rom {
    fn rom(&self) -> &crate::rom::Rom;
}

#[derive(Serialize, Deserialize, Delegate)]
#[delegate(Ppu, target = "inner")]
#[delegate(Apu, target = "inner")]
#[delegate(InterruptFlag, target = "inner")]
pub struct Context {
    pub bus: crate::bus::Bus,
    #[serde(flatten)]
    pub inner: BusContext,
}

#[derive(Serialize, Deserialize, Delegate)]
#[delegate(Apu, target = "inner")]
#[delegate(Oam, target = "inner")]
#[delegate(Vram, target = "inner")]
#[delegate(InterruptFlag, target = "inner")]
pub struct BusContext {
    #[serde(skip)]
    pub rom: crate::rom::Rom,
    pub ppu: crate::ppu::Ppu,
    #[serde(flatten)]
    pub inner: PpuContext,
}

#[derive(Serialize, Deserialize)]
pub struct PpuContext {
    pub apu: crate::apu::Apu,
    pub vram: Vec<u8>,
    pub vram_lock: bool,
    pub oam: Vec<u8>,
    pub oam_lock: bool,
    pub interrupt_enable: u8,
    pub interrupt_flag: u8,
}

impl Context {
    pub fn new(
        bus: crate::bus::Bus,
        rom: crate::rom::Rom,
        ppu: crate::ppu::Ppu,
        apu: crate::apu::Apu,
    ) -> Self {
        Self {
            bus,
            inner: BusContext {
                rom,
                ppu,
                inner: PpuContext {
                    apu,
                    vram: vec![0; 0x2000],
                    vram_lock: false,
                    oam: vec![0; 0xA0],
                    oam_lock: false,
                    interrupt_enable: 0,
                    interrupt_flag: 0,
                },
            },
        }
    }
}

impl Bus for Context {
    fn tick(&mut self) {
        self.bus.tick(&mut self.inner);
        for _ in 0..4 {
            self.inner.ppu.tick(&mut self.inner.inner);
            self.inner.inner.apu.tick();
        }
        self.bus.io().serial().tick(&mut self.inner);
        self.bus.io().tick(&mut self.inner);
    }

    fn read(&mut self, addr: u16) -> u8 {
        self.bus.read(&mut self.inner, addr)
    }

    fn read_immutable(&mut self, addr: u16) -> Option<u8> {
        self.bus.read_immutable(&mut self.inner, addr)
    }

    fn write(&mut self, addr: u16, data: u8) {
        self.bus.write(&mut self.inner, addr, data)
    }
}

impl Rom for BusContext {
    fn rom(&self) -> &crate::rom::Rom {
        &self.rom
    }
}

impl Ppu for BusContext {
    fn ppu(&self) -> &crate::ppu::Ppu {
        &self.ppu
    }
    fn ppu_mut(&mut self) -> &mut crate::ppu::Ppu {
        &mut self.ppu
    }
}

impl Apu for PpuContext {
    fn apu(&self) -> &crate::apu::Apu {
        &self.apu
    }
    fn apu_mut(&mut self) -> &mut crate::apu::Apu {
        &mut self.apu
    }
}

impl InterruptFlag for PpuContext {
    fn interrupt_enable(&mut self) -> u8 {
        self.interrupt_enable
    }
    fn set_interrupt_enable(&mut self, data: u8) {
        self.interrupt_enable = data;
    }
    fn interrupt_flag(&mut self) -> u8 {
        self.interrupt_flag
    }
    fn set_interrupt_flag(&mut self, data: u8) {
        self.interrupt_flag = data;
    }
}

impl Vram for PpuContext {
    fn read_vram(&self, addr: u16, force: bool) -> u8 {
        if force || !self.vram_lock {
            self.vram[addr as usize]
        } else {
            !0
        }
    }
    fn write_vram(&mut self, addr: u16, data: u8, force: bool) {
        if force || !self.vram_lock {
            self.vram[addr as usize] = data;
        }
    }
    fn lock_vram(&mut self, lock: bool) {
        self.vram_lock = lock;
    }
}

impl Oam for PpuContext {
    fn read_oam(&self, addr: u8, force: bool) -> u8 {
        if force || !self.oam_lock {
            self.oam[addr as usize]
        } else {
            !0
        }
    }
    fn write_oam(&mut self, addr: u8, data: u8, force: bool) {
        if force || !self.oam_lock {
            self.oam[addr as usize] = data;
        }
    }
    fn lock_oam(&mut self, lock: bool) {
        self.oam_lock = lock;
    }
}
