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
}

pub trait Vram {
    fn read_vram(&self, addr: u16) -> u8;
    fn write_vram(&mut self, addr: u16, data: u8);
    fn lock_vram(&mut self, lock: bool);
}

pub trait Oam {
    fn read_oam(&self, addr: u8) -> u8;
    fn write_oam(&mut self, addr: u8, data: u8);
    fn lock_oam(&mut self, lock: bool);
}
