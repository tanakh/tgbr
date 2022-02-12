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
pub trait Ppu {
    fn ppu(&self) -> &crate::ppu::Ppu;
    fn ppu_mut(&mut self) -> &mut crate::ppu::Ppu;
}

#[delegatable_trait]
pub trait Apu {
    fn apu(&self) -> &crate::apu::Apu;
    fn apu_mut(&mut self) -> &mut crate::apu::Apu;
}

#[delegatable_trait]
pub trait Rom {
    fn rom(&self) -> &crate::rom::Rom;
    fn rom_mut(&mut self) -> &mut crate::rom::Rom;
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

impl Context {
    pub fn new(
        bus: crate::bus::Bus,
        rom: crate::rom::Rom,
        ppu: crate::ppu::Ppu,
        apu: crate::apu::Apu,
    ) -> Self {
        Self {
            bus,
            inner: InnerContext1 {
                rom,
                ppu,
                apu,
                inner: InnerContext2 {
                    interrupt_enable: 0,
                    interrupt_flag: 0,
                },
            },
        }
    }
}

#[derive(Serialize, Deserialize, Delegate)]
#[delegate(Rom, target = "inner")]
#[delegate(Ppu, target = "inner")]
#[delegate(Apu, target = "inner")]
#[delegate(InterruptFlag, target = "inner")]
pub struct Context {
    pub bus: crate::bus::Bus,
    #[serde(flatten)]
    pub inner: InnerContext1,
}

impl Bus for Context {
    fn tick(&mut self) {
        self.bus.tick(&mut self.inner);
        for _ in 0..4 {
            self.inner.ppu.tick(&mut self.inner.inner);
            self.inner.apu.tick();
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

#[derive(Serialize, Deserialize, Delegate)]
#[delegate(InterruptFlag, target = "inner")]
pub struct InnerContext1 {
    #[serde(skip)]
    rom: crate::rom::Rom,
    ppu: crate::ppu::Ppu,
    apu: crate::apu::Apu,
    #[serde(flatten)]
    inner: InnerContext2,
}

impl Rom for InnerContext1 {
    fn rom(&self) -> &crate::rom::Rom {
        &self.rom
    }
    fn rom_mut(&mut self) -> &mut crate::rom::Rom {
        &mut self.rom
    }
}

impl Ppu for InnerContext1 {
    fn ppu(&self) -> &crate::ppu::Ppu {
        &self.ppu
    }
    fn ppu_mut(&mut self) -> &mut crate::ppu::Ppu {
        &mut self.ppu
    }
}

impl Apu for InnerContext1 {
    fn apu(&self) -> &crate::apu::Apu {
        &self.apu
    }
    fn apu_mut(&mut self) -> &mut crate::apu::Apu {
        &mut self.apu
    }
}

#[derive(Serialize, Deserialize)]
pub struct InnerContext2 {
    interrupt_enable: u8,
    interrupt_flag: u8,
}

impl InterruptFlag for InnerContext2 {
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
