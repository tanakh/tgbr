use crate::rom;

pub trait Bus {
    fn tick(&mut self);
    fn read(&mut self, addr: u16) -> u8;
    fn read_immutable(&mut self, addr: u16) -> Option<u8>;
    fn write(&mut self, addr: u16, data: u8);
}

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

pub trait Vram {
    fn read_vram(&self, addr: u16, force: bool) -> u8;
    fn write_vram(&mut self, addr: u16, data: u8, force: bool);
    fn lock_vram(&mut self, lock: bool);
}

pub trait Oam {
    fn read_oam(&self, addr: u8, force: bool) -> u8;
    fn write_oam(&mut self, addr: u8, data: u8, force: bool);
    fn lock_oam(&mut self, lock: bool);
}

pub trait Ppu {
    fn read_ppu(&mut self, addr: u16) -> u8;
    fn write_ppu(&mut self, addr: u16, data: u8);
}

pub trait Apu {
    fn read_apu(&mut self, addr: u16) -> u8;
    fn write_apu(&mut self, addr: u16, data: u8);
}

pub trait Rom {
    fn rom(&self) -> &rom::Rom;
}
