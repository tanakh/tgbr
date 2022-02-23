use ambassador::{delegatable_trait, Delegate};
use serde::{Deserialize, Serialize};

use crate::{interface::Color, mbc::create_mbc};

#[delegatable_trait]
pub trait Bus {
    fn tick(&mut self);
    fn stop(&mut self);
    fn read(&mut self, addr: u16) -> u8;
    fn read_immutable(&mut self, addr: u16) -> Option<u8>;
    fn write(&mut self, addr: u16, data: u8);
}

#[delegatable_trait]
pub trait Vram {
    fn vram(&self) -> &[u8];
    fn vram_mut(&mut self) -> &mut [u8];
    fn vram_lock(&self) -> bool;
    fn set_vram_lock(&mut self, lock: bool);
}

#[delegatable_trait]
pub trait Oam {
    fn oam(&self) -> &[u8];
    fn oam_mut(&mut self) -> &mut [u8];
    fn oam_lock(&self) -> bool;
    fn set_oam_lock(&mut self, lock: bool);
}

#[delegatable_trait]
pub trait Ppu {
    fn read_ppu(&mut self, addr: u16) -> u8;
    fn write_ppu(&mut self, addr: u16, data: u8);
    fn mode(&self) -> crate::ppu::Mode;
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
pub trait Model {
    fn model(&self) -> crate::config::Model;
}

#[delegatable_trait]
pub trait InterruptFlag {
    fn interrupt_enable(&mut self) -> u8;
    fn set_interrupt_enable(&mut self, data: u8);
    fn interrupt_flag(&mut self) -> u8;
    fn set_interrupt_flag(&mut self, data: u8);
    fn stall_cpu(&mut self, cycle: usize);
    fn check_stall_cpu(&mut self) -> bool;

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
        model: crate::config::Model,
        rom: crate::rom::Rom,
        boot_rom: &Option<Vec<u8>>,
        backup_ram: Option<Vec<u8>>,
        dmg_palette: &[Color; 4],
    ) -> Self {
        let io = crate::io::Io::new();
        let mbc = create_mbc(&rom, backup_ram);
        let bus = crate::bus::Bus::new(model, mbc, boot_rom, io);
        let vram_size = if model.is_cgb() { 0x4000 } else { 0x2000 };

        Self {
            cpu: crate::cpu::Cpu::new(),
            inner: InnerContext0 {
                bus,
                inner: InnerContext1 {
                    rom,
                    ppu: crate::ppu::Ppu::new(dmg_palette),
                    apu: crate::apu::Apu::new(),
                    inner: InnerContext2 {
                        model,
                        vram: vec![0; vram_size],
                        vram_lock: false,
                        oam: vec![0; 0xA0],
                        oam_lock: false,
                        interrupt_enable: 0,
                        interrupt_flag: 0,
                        stall_cpu: 0,
                    },
                },
            },
        }
    }
}

#[derive(Serialize, Deserialize, Delegate)]
#[delegate(Rom, target = "inner")]
#[delegate(Ppu, target = "inner")]
#[delegate(Apu, target = "inner")]
#[delegate(Model, target = "inner")]
#[delegate(Vram, target = "inner")]
#[delegate(Oam, target = "inner")]
#[delegate(InterruptFlag, target = "inner")]
pub struct Context {
    pub cpu: crate::cpu::Cpu,
    #[serde(flatten)]
    pub inner: InnerContext0,
}

#[derive(Serialize, Deserialize, Delegate)]
#[delegate(Rom, target = "inner")]
#[delegate(Ppu, target = "inner")]
#[delegate(Apu, target = "inner")]
#[delegate(Model, target = "inner")]
#[delegate(Vram, target = "inner")]
#[delegate(Oam, target = "inner")]
#[delegate(InterruptFlag, target = "inner")]
pub struct InnerContext0 {
    pub bus: crate::bus::Bus,
    #[serde(flatten)]
    pub inner: InnerContext1,
}

impl Bus for InnerContext0 {
    fn tick(&mut self) {
        self.bus.tick(&mut self.inner);
        let speed = self.bus.current_speed();
        for _ in 0..if speed == 0 { 4 } else { 2 } {
            self.inner.ppu.tick(&mut self.inner.inner);
            self.inner.apu.tick();
        }
        self.bus.io().serial().tick(&mut self.inner);
        self.bus.io().tick(&mut self.inner);
    }

    fn stop(&mut self) {
        self.bus.stop();
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
#[delegate(Model, target = "inner")]
#[delegate(Vram, target = "inner")]
#[delegate(Oam, target = "inner")]
#[delegate(InterruptFlag, target = "inner")]
pub struct InnerContext1 {
    #[serde(skip)]
    pub rom: crate::rom::Rom,
    pub ppu: crate::ppu::Ppu,
    pub apu: crate::apu::Apu,
    #[serde(flatten)]
    pub inner: InnerContext2,
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
    fn read_ppu(&mut self, addr: u16) -> u8 {
        self.ppu.read(&mut self.inner, addr)
    }
    fn write_ppu(&mut self, addr: u16, data: u8) {
        self.ppu.write(&mut self.inner, addr, data)
    }
    fn mode(&self) -> crate::ppu::Mode {
        self.ppu.mode()
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
    model: crate::config::Model,
    #[serde(with = "serde_bytes")]
    vram: Vec<u8>,
    vram_lock: bool,
    #[serde(with = "serde_bytes")]
    oam: Vec<u8>,
    oam_lock: bool,
    interrupt_enable: u8,
    interrupt_flag: u8,
    stall_cpu: usize,
}

impl Model for InnerContext2 {
    fn model(&self) -> crate::config::Model {
        self.model
    }
}

impl Vram for InnerContext2 {
    fn vram(&self) -> &[u8] {
        &self.vram
    }
    fn vram_mut(&mut self) -> &mut [u8] {
        &mut self.vram
    }
    fn vram_lock(&self) -> bool {
        self.vram_lock
    }
    fn set_vram_lock(&mut self, lock: bool) {
        self.vram_lock = lock;
    }
}

impl Oam for InnerContext2 {
    fn oam(&self) -> &[u8] {
        &self.oam
    }
    fn oam_mut(&mut self) -> &mut [u8] {
        &mut self.oam
    }
    fn oam_lock(&self) -> bool {
        self.oam_lock
    }
    fn set_oam_lock(&mut self, lock: bool) {
        self.oam_lock = lock;
    }
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
    fn stall_cpu(&mut self, cycle: usize) {
        self.stall_cpu += cycle;
    }
    fn check_stall_cpu(&mut self) -> bool {
        if self.stall_cpu > 0 {
            self.stall_cpu -= 1;
            true
        } else {
            false
        }
    }
}
