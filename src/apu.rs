use bitvec::prelude::*;
use log::trace;

#[derive(Default)]
pub struct Apu {
    pulse: [Pulse; 2],
    wave: Wave,
    noise: Noise,
    channel_ctrl: [ChannelCtrl; 2], // 0=right, 1=left
    all_sound_on: bool,
}

#[derive(Default)]
struct Pulse {
    sweep_time: u8,
    sweep_dec: bool,
    sweep_shift: u8,
    duty: u8,
    length: u8, // Sound Length = (64-t1)*(1/256) seconds
    initial_volume: u8,
    envelope_inc: bool,
    envelope_time: u8, // Length of 1 step = n*(1/64) sec
    frequency: u16,    // Frequency = 131072/(2048-x) Hz
    stop_when_length_expires: bool,
}

#[derive(Default)]
struct Wave {
    enable: bool,
    length: u8,       // Sound Length = (256-t1)*(1/256) seconds
    output_level: u8, // 0 => mute, 1 => 100%, 2 => 50%, 3 => 25%
    frequency: u16,
    stop_when_length_expires: bool,
    ram: [u8; 0x10],
}

#[derive(Default)]
struct Noise {
    length: u8, // Sound Length = (64-t1)*(1/256) seconds
    initial_volume: u8,
    envelope_inc: bool,
    envelope_time: u8, // Length of 1 step = n*(1/64) sec
    shift_clock_frequency: u8,
    counter_width: bool, // false=15bits, true=7bits
    dividing_ratio: u8,  // Frequency = 524288 Hz / dividing_ratio / 2^(shift_clock_frequency+1)
    stop_when_length_expires: bool,
}

#[derive(Default)]
struct ChannelCtrl {
    output: bool,
    volume: u8,
    output_ch: [bool; 4],
}

impl Apu {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn read(&mut self, addr: u16) -> u8 {
        let data = match addr & 0xff {
            _ => todo!(),
        };
        trace!("Read from APU register: {addr:04X} = {data:02X}");
        data
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        trace!("Write to APU register: {addr:04X} = {data:02X}");
        match addr & 0xff {
            // NR10: Channel 1 Sweep register (R/W)
            0x10 => {
                let v = data.view_bits::<Lsb0>();
                self.pulse[0].sweep_time = v[4..=6].load();
                self.pulse[0].sweep_dec = v[3];
                self.pulse[0].sweep_shift = v[0..=2].load();
            }
            // NR11/NR21: Channel1/2 Sound length/Wave pattern duty (R/W)
            0x11 | 0x16 => {
                let ch = (addr & 0xf) as usize / 5;
                let v = data.view_bits::<Lsb0>();
                self.pulse[ch].duty = v[6..=7].load();
                self.pulse[ch].length = v[0..=5].load();
            }
            // NR12/NR22: Channel 1/2 Volume Envelope (R/W)
            0x12 | 0x17 => {
                let ch = (addr & 0xf) as usize / 5;
                let v = data.view_bits::<Lsb0>();
                self.pulse[ch].initial_volume = v[4..=7].load();
                self.pulse[ch].envelope_inc = v[3];
                self.pulse[ch].envelope_time = v[0..=2].load();
            }
            // NR13/NR23: Channel 1 Frequency lo (W)
            0x13 | 0x18 => {
                let ch = (addr & 0xf) as usize / 5;
                self.pulse[ch].frequency.view_bits_mut::<Lsb0>()[0..7].store(data);
            }
            // NR14/NR24: Channel 1 Frequency hi (R/W)
            0x14 | 0x19 => {
                let ch = (addr & 0xf) as usize / 5;
                let v = data.view_bits::<Lsb0>();
                self.pulse[ch].frequency.view_bits_mut::<Lsb0>()[8..=10]
                    .store(v[0..=2].load::<u16>());
                self.pulse[ch].stop_when_length_expires = v[6];
                // Restart sound
                if v[7] {
                    todo!();
                }
            }
            // NR30: Channel 3 Sound on/off (R/W)
            0x1A => self.wave.enable = data.view_bits::<Lsb0>()[7],
            // NR31: Channel 3 Sound length (R/W)
            0x1B => self.wave.length = data,
            // NR32: Channel 3 Select output level (R/W)
            0x1C => self.wave.output_level = data.view_bits::<Lsb0>()[5..=6].load(),
            // NR33: Channel 3 Frequency lo (R/W)
            0x1D => self.wave.frequency.view_bits_mut::<Lsb0>()[0..7].store(data),
            // NR34: Channel 3 Frequency hi (R/W)
            0x1E => {
                let v = data.view_bits::<Lsb0>();
                self.wave.frequency.view_bits_mut::<Lsb0>()[8..=10].store(v[0..=2].load::<u16>());
                self.wave.stop_when_length_expires = v[6];
                // Restart sound
                if v[7] {
                    todo!();
                }
            }
            // NR41: Channel 4 Sound length (R/W)
            0x20 => self.noise.length = data.view_bits::<Lsb0>()[0..=5].load(),
            // NR42: Channel 4 Volume Envelope (R/W)
            0x21 => {
                let v = data.view_bits::<Lsb0>();
                self.noise.initial_volume = v[4..=7].load();
                self.noise.envelope_inc = v[3];
                self.noise.envelope_time = v[0..=2].load();
            }
            // NR43: Channel 4 Polynomial Counter (R/W)
            0x22 => {
                let v = data.view_bits::<Lsb0>();
                self.noise.shift_clock_frequency = v[4..=7].load();
                self.noise.counter_width = v[3];
                self.noise.dividing_ratio = v[0..=2].load();
            }
            // NR44: Channel 4 Counter/consecutive; initial (R/W)
            0x23 => {
                let v = data.view_bits::<Lsb0>();
                self.noise.stop_when_length_expires = v[6];
                // Restart sound
                if v[7] {
                    todo!();
                }
            }

            // NR50: Channel control / ON-OFF / Volume (R/W)
            0x24 => {
                let v = data.view_bits::<Lsb0>();
                self.channel_ctrl[0].output = v[7];
                self.channel_ctrl[0].volume = v[4..=6].load();
                self.channel_ctrl[1].output = v[3];
                self.channel_ctrl[1].volume = v[0..=2].load();
            }
            // NR51: Selection of Sound output terminal (R/W)
            0x25 => {
                let v = data.view_bits::<Lsb0>();
                self.channel_ctrl[0].output_ch[3] = v[7];
                self.channel_ctrl[0].output_ch[2] = v[6];
                self.channel_ctrl[0].output_ch[1] = v[5];
                self.channel_ctrl[0].output_ch[0] = v[4];
                self.channel_ctrl[1].output_ch[3] = v[3];
                self.channel_ctrl[1].output_ch[2] = v[2];
                self.channel_ctrl[1].output_ch[1] = v[1];
                self.channel_ctrl[1].output_ch[0] = v[0];
            }
            // NR52: Sound on/off (R/W)
            0x26 => self.all_sound_on = data.view_bits::<Lsb0>()[7],

            // Wave Pattern RAM
            0x30..=0x3F => self.wave.ram[(addr & 0xff) as usize - 0x30] = data,

            _ => todo!(),
        }
    }
}
