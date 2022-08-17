use bitvec::prelude::*;
use log::trace;
use meru_interface::{AudioBuffer, AudioSample};
use serde::{Deserialize, Serialize};
use std::cmp::min;

use crate::{
    consts::{AUDIO_SAMPLE_PER_FRAME, DOTS_PER_LINE, LINES_PER_FRAME},
    util::{pack, ClockDivider},
};

#[derive(Default, Serialize, Deserialize)]
pub struct Apu {
    pulse: [Pulse; 2],
    wave: Wave,
    noise: Noise,

    channel_ctrl: [ChannelCtrl; 2], // 0=right, 1=left
    power_on: bool,

    frame_sequencer_div: ClockDivider,
    frame_sequencer_step: u64,
    sampling_counter: u64,

    #[serde(skip)]
    audio_buffer: AudioBuffer,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct ChannelCtrl {
    vin_enable: bool,
    volume: u8,
    output_ch: [bool; 4],
}

impl Apu {
    pub fn new() -> Self {
        Self {
            pulse: [Pulse::new(true), Pulse::new(false)],
            frame_sequencer_step: 7,
            frame_sequencer_div: ClockDivider::with_period(8192),
            ..Default::default()
        }
    }

    fn set_power(&mut self, on: bool) {
        if self.power_on == on {
            return;
        }

        self.power_on = on;
        self.frame_sequencer_div.reset();
        self.frame_sequencer_step = 7;
        self.channel_ctrl = Default::default();

        // except on the DMG, where length counters are unaffected by power and can still be written while off
        // FIXME: CGB mode

        let ch1_len = self.pulse[0].length;
        let ch2_len = self.pulse[1].length;
        let ch3_len = self.wave.length;
        let ch4_len = self.noise.length;

        self.pulse[0].reset();
        self.pulse[1].reset();
        self.wave.reset();
        self.noise.reset();

        self.pulse[0].length = ch1_len;
        self.pulse[1].length = ch2_len;
        self.wave.length = ch3_len;
        self.noise.length = ch4_len;
    }

    pub fn audio_buffer(&self) -> &AudioBuffer {
        &self.audio_buffer
    }

    pub fn audio_buffer_mut(&mut self) -> &mut AudioBuffer {
        &mut self.audio_buffer
    }

    pub fn tick(&mut self) {
        if self.power_on {
            if self.frame_sequencer_div.tick() {
                self.frame_sequencer_step += 1;
            }

            // Step   Length Ctr  Vol Env     Sweep
            // ---------------------------------------
            // 0      Clock       -           -
            // 1      -           -           -
            // 2      Clock       -           Clock
            // 3      -           -           -
            // 4      Clock       -           -
            // 5      -           -           -
            // 6      Clock       -           Clock
            // 7      -           Clock       -
            // ---------------------------------------
            // Rate   256 Hz      64 Hz       128 Hz

            let length_tick = self.frame_sequencer_step % 2 == 0;
            let envelope_tick = self.frame_sequencer_step % 8 == 7;
            let sweep_tick = self.frame_sequencer_step % 4 == 2;

            self.pulse[0].tick(length_tick, envelope_tick, sweep_tick);
            self.pulse[1].tick(length_tick, envelope_tick, false);
            self.wave.tick(length_tick);
            self.noise.tick(length_tick, envelope_tick);
        }

        // AUDIO_SAMPLE_PER_FRAME samples per DOTS_PER_LINE * LINES_PER_FRAME
        const TICKS_PER_SECOND: u64 = DOTS_PER_LINE * LINES_PER_FRAME;

        self.sampling_counter += AUDIO_SAMPLE_PER_FRAME;
        if self.sampling_counter >= TICKS_PER_SECOND {
            self.sampling_counter -= TICKS_PER_SECOND;
            let sample = self.mix_output();
            self.audio_buffer.samples.push(sample);
        }
    }

    fn mix_output(&mut self) -> AudioSample {
        if !self.power_on {
            return AudioSample::new(0, 0);
        }

        let dac = |output: Option<u8>| match output {
            None => 0,
            Some(output) => (output as i16 * 1000 - 7500) / 8,
        };

        let ch_output = [
            dac(self.pulse[0].output()),
            dac(self.pulse[1].output()),
            dac(self.wave.output()),
            dac(self.noise.output()),
        ];

        let mut output = [0, 0];

        for (i, out) in output.iter_mut().enumerate() {
            for (j, ch_out) in ch_output.iter().enumerate() {
                if self.channel_ctrl[i].output_ch[j] {
                    *out += *ch_out;
                }
            }
            *out *= self.channel_ctrl[i].volume as i16 + 1;
        }

        AudioSample::new(output[0], output[1])
    }
}

#[rustfmt::skip]
const REGISTER_NAME: &[&str] = &[
    "NR10", "NR11", "NR12", "NR13", "NR14",
    "????", "NR21", "NR22", "NR23", "NR24",
    "NR30", "NR31", "NR32", "NR33", "NR34",
    "????", "NR41", "NR42", "NR43", "NR44",
    "NR50", "NR51", "NR52",
    "????", "????", "????", "????", "????", "????", "????", "????", "????",
];

fn register_name(addr: u16) -> String {
    match addr {
        0xFF10..=0xFF2F => REGISTER_NAME[(addr - 0xFF10) as usize].to_string(),
        _ => format!("WAVE[${:X}]", addr - 0xFF30),
    }
}

impl Apu {
    pub fn read(&mut self, addr: u16) -> u8 {
        let data = match addr {
            0xFF10..=0xFF14 => self.pulse[0].read((addr - 0xFF10) as usize),
            0xFF15 => !0,
            0xFF16..=0xFF19 => self.pulse[1].read((addr - 0xFF15) as usize),
            0xFF1A..=0xFF1E => self.wave.read((addr - 0xFF1A) as usize),
            0xFF1F => !0,
            0xFF20..=0xFF23 => self.noise.read((addr - 0xFF20) as usize),

            // NR50: Channel control / ON-OFF / Volume (R/W)
            0xFF24 => pack! {
                7     => self.channel_ctrl[1].vin_enable,
                4..=6 => self.channel_ctrl[1].volume,
                3     => self.channel_ctrl[0].vin_enable,
                0..=2 => self.channel_ctrl[0].volume,
            },
            // NR51: Selection of Sound output terminal (R/W)
            0xFF25 => pack! {
                7 => self.channel_ctrl[1].output_ch[3],
                6 => self.channel_ctrl[1].output_ch[2],
                5 => self.channel_ctrl[1].output_ch[1],
                4 => self.channel_ctrl[1].output_ch[0],
                3 => self.channel_ctrl[0].output_ch[3],
                2 => self.channel_ctrl[0].output_ch[2],
                1 => self.channel_ctrl[0].output_ch[1],
                0 => self.channel_ctrl[0].output_ch[0],
            },
            // NR52: Sound on/off (R/W)
            0xFF26 => pack! {
                7 => self.power_on,
                4..=6 => !0,
                3 => self.noise.on,
                2 => self.wave.on,
                1 => self.pulse[1].on,
                0 => self.pulse[0].on,
            },

            0xFF27..=0xFF2F => !0,

            // Wave Pattern RAM
            0xFF30..=0xFF3F => self.wave.ram[(addr & 0xf) as usize],

            // PCM12
            0xFF76 => pack! {
                4..=7 => self.pulse[1].output().unwrap_or(0),
                0..=3 => self.pulse[0].output().unwrap_or(0),
            },
            // PCM34
            0xFF77 => pack! {
                4..=7 => self.noise.output().unwrap_or(0),
                0..=3 => self.wave.output().unwrap_or(0),
            },

            _ => unreachable!(),
        };
        trace!(
            "Read from APU register: {}( = ${addr:04X}) = ${data:02X}",
            register_name(addr),
        );
        data
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        trace!(
            "Write to APU register: {}(= ${addr:04X}) = ${data:02X}",
            register_name(addr),
        );
        match addr {
            0xFF10..=0xFF14 => self.pulse[0].write((addr - 0xFF10) as usize, data),
            0xFF15 => {}
            0xFF16..=0xFF19 => self.pulse[1].write((addr - 0xFF15) as usize, data),
            0xFF1A..=0xFF1E => self.wave.write((addr - 0xFF1A) as usize, data),
            0xFF1F => {}
            0xFF20..=0xFF23 => self.noise.write((addr - 0xFF20) as usize, data),

            // NR50: Channel control / ON-OFF / Volume (R/W)
            0xFF24 => {
                let v = data.view_bits::<Lsb0>();
                self.channel_ctrl[1].vin_enable = v[7];
                self.channel_ctrl[1].volume = v[4..=6].load();
                self.channel_ctrl[0].vin_enable = v[3];
                self.channel_ctrl[0].volume = v[0..=2].load();
            }
            // NR51: Selection of Sound output terminal (R/W)
            0xFF25 => {
                let v = data.view_bits::<Lsb0>();
                self.channel_ctrl[1].output_ch[3] = v[7];
                self.channel_ctrl[1].output_ch[2] = v[6];
                self.channel_ctrl[1].output_ch[1] = v[5];
                self.channel_ctrl[1].output_ch[0] = v[4];
                self.channel_ctrl[0].output_ch[3] = v[3];
                self.channel_ctrl[0].output_ch[2] = v[2];
                self.channel_ctrl[0].output_ch[1] = v[1];
                self.channel_ctrl[0].output_ch[0] = v[0];
            }
            // NR52: Sound on/off (R/W)
            0xFF26 => self.set_power(data.view_bits::<Lsb0>()[7]),
            0xFF27..=0xFF2F => {}

            // Wave Pattern RAM
            0xFF30..=0xFF3F => self.wave.ram[(addr & 0xf) as usize] = data,

            // PCM12 - PCM amplitudes 1 & 2 (Read Only)
            0xFF76 => {}
            // PCM34 - PCM amplitudes 3 & 4 (Read Only)
            0xFF77 => {}

            _ => unreachable!(),
        }
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct Pulse {
    has_sweep_unit: bool,
    sweep_period: u8,
    sweep_negate: bool,
    sweep_shift: u8,

    duty: u8,
    length: u8, // Sound Length = (64-t1)*(1/256) seconds
    initial_volume: u8,
    envelope_inc: bool,
    envelope_period: u8, // Length of 1 step = n*(1/64) sec
    frequency: u16,      // Frequency = 131072/(2048-x) Hz
    length_enable: bool,

    on: bool,
    current_volume: u8,
    envelope_timer: u8,

    sweep_enable: bool,
    freq_calculated_in_negate_mode: bool,
    current_frequency: u16,
    sweep_timer: u8,

    frequency_timer: u16,
    phase: usize,

    length_tick_in: bool,
    prev_length_tick: bool,
    envelope_tick_in: bool,
    prev_envelope_tick: bool,
    sweep_tick_in: bool,
    prev_sweep_tick: bool,
}

impl Pulse {
    fn new(has_sweeep_unit: bool) -> Self {
        Self {
            has_sweep_unit: has_sweeep_unit,
            ..Default::default()
        }
    }

    fn reset(&mut self) {
        *self = Self::new(self.has_sweep_unit);
    }

    // Square 1
    // NR10 FF10 -PPP NSSS Sweep period, negate, shift
    // NR11 FF11 DDLL LLLL Duty, Length load (64-L)
    // NR12 FF12 VVVV APPP Starting volume, Envelope add mode, period
    // NR13 FF13 FFFF FFFF Frequency LSB
    // NR14 FF14 TL-- -FFF Trigger, Length enable, Frequency MSB

    // Square 2
    //      FF15 ---- ---- Not used
    // NR21 FF16 DDLL LLLL Duty, Length load (64-L)
    // NR22 FF17 VVVV APPP Starting volume, Envelope add mode, period
    // NR23 FF18 FFFF FFFF Frequency LSB
    // NR24 FF19 TL-- -FFF Trigger, Length enable, Frequency MSB

    fn read(&mut self, regno: usize) -> u8 {
        match regno {
            // NR10: Channel 1 Sweep register (R/W)
            0 => pack! {
                7     => true,
                4..=6 => self.sweep_period,
                3     => self.sweep_negate,
                0..=2 => self.sweep_shift,
            },
            // NR11/NR21: Channel 1/2 Sound length/Wave pattern duty (R/W)
            // Only bits 7-6 can be read.
            1 => pack!(6..=7 => self.duty, 0..=5 => !0),
            // NR12/NR22: Channel 1/2 Envelope (R/W)
            2 => pack! {
                4..=7 => self.initial_volume,
                3     => self.envelope_inc,
                0..=2 => self.envelope_period,
            },
            // NR13/NR23: Channel 1/2 Frequency lo (W)
            3 => !0,
            // NR14/NR24: Channel 1/2 Frequency hi (R/W)
            // Only bit 6 can be read
            4 => pack! {
                7 => true,
                6 => self.length_enable,
                0..=5 => !0,
            },
            _ => unreachable!(),
        }
    }

    fn write(&mut self, regno: usize, data: u8) {
        match regno {
            // NR10: Channel 1 Sweep register (R/W)
            0 => {
                let prev_sweep_negate = self.sweep_negate;

                let v = data.view_bits::<Lsb0>();
                self.sweep_period = v[4..=6].load();
                self.sweep_negate = v[3];
                self.sweep_shift = v[0..=2].load();

                // neg -> pos after freq calculation disables channel
                if self.sweep_enable
                    && self.freq_calculated_in_negate_mode
                    && (prev_sweep_negate && !self.sweep_negate)
                {
                    self.on = false;
                    self.freq_calculated_in_negate_mode = false;
                }
            }
            // NR11/NR21: Channel1/2 Sound length/Wave pattern duty (R/W)
            1 => {
                let v = data.view_bits::<Lsb0>();
                self.duty = v[6..=7].load();
                self.length = 64 - v[0..=5].load::<u8>();
            }
            // NR12/NR22: Channel 1/2 Volume Envelope (R/W)
            2 => {
                let v = data.view_bits::<Lsb0>();
                self.initial_volume = v[4..=7].load();
                self.envelope_inc = v[3];
                self.envelope_period = v[0..=2].load();
            }
            // NR13/NR23: Channel 1/2 Frequency lo (W)
            3 => self.frequency.view_bits_mut::<Lsb0>()[0..=7].store(data),
            // NR14/NR24: Channel 1/2 Frequency hi (R/W)
            4 => {
                let v = data.view_bits::<Lsb0>();
                self.frequency.view_bits_mut::<Lsb0>()[8..=10].store(v[0..=2].load::<u16>());

                let prev_length_enable = self.length_enable;
                self.length_enable = v[6];

                // Extra length clogking
                if self.length_tick_in && !prev_length_enable && self.length_enable {
                    self.length_tick();
                }

                if v[7] {
                    self.trigger();
                }
            }
            _ => unreachable!(),
        }
        if !self.dac_enable() {
            self.on = false;
        }
    }

    fn dac_enable(&self) -> bool {
        self.initial_volume > 0 || self.envelope_inc
    }

    fn trigger(&mut self) {
        self.on = self.dac_enable();
        self.freq_calculated_in_negate_mode = false;

        if self.length == 0 {
            self.length = 64;
            if self.length_tick_in && self.length_enable {
                self.length_tick();
            }
        }

        self.frequency_timer = (2048 - self.frequency) * 4;

        // The volume envelope and sweep timers treat a period of 0 as 8.
        self.envelope_timer = if self.envelope_period == 0 {
            8
        } else {
            self.envelope_period
        };
        self.current_volume = self.initial_volume;

        self.sweep_timer = self.sweep_period();
        self.sweep_enable = self.sweep_period != 0 || self.sweep_shift != 0;
        self.current_frequency = self.frequency;
        if self.sweep_shift != 0 && self.new_freq() > 2047 {
            self.on = false;
        }
    }

    fn new_freq(&mut self) -> u16 {
        if self.sweep_negate {
            self.freq_calculated_in_negate_mode = true;
            self.current_frequency - (self.current_frequency >> self.sweep_shift)
        } else {
            self.current_frequency + (self.current_frequency >> self.sweep_shift)
        }
    }

    fn sweep_period(&self) -> u8 {
        if self.sweep_period == 0 {
            8
        } else {
            self.sweep_period
        }
    }

    fn tick(&mut self, length_tick: bool, envelope_tick: bool, sweep_tick: bool) {
        self.frequency_timer = self.frequency_timer.saturating_sub(1);
        if self.frequency_timer == 0 {
            self.frequency_timer = (2048 - self.frequency) * 4;
            self.phase = (self.phase + 1) % 8;
        }

        self.length_tick_in = length_tick;
        self.envelope_tick_in = envelope_tick;
        self.sweep_tick_in = sweep_tick;

        self.update_tick();
    }

    fn update_tick(&mut self) {
        let length_tick = self.length_tick_in;
        if !self.prev_length_tick && length_tick && self.length_enable {
            self.length_tick();
        }
        self.prev_length_tick = length_tick;

        let envelope_tick = self.envelope_tick_in;
        if !self.prev_envelope_tick && envelope_tick {
            self.envelope_tick();
        }
        self.prev_envelope_tick = envelope_tick;

        let sweep_tick = self.sweep_tick_in;
        if !self.prev_sweep_tick && sweep_tick {
            self.sweep_tick();
        }
        self.prev_sweep_tick = sweep_tick;
    }

    fn length_tick(&mut self) {
        self.length = self.length.saturating_sub(1);
        if self.length == 0 {
            self.on = false;
        }
    }

    fn envelope_tick(&mut self) {
        if self.envelope_timer > 0 {
            self.envelope_timer -= 1;
            if self.envelope_timer == 0 && self.envelope_period > 0 {
                self.envelope_timer = self.envelope_period;

                if self.envelope_inc {
                    self.current_volume = min(15, self.current_volume + 1);
                } else {
                    self.current_volume = self.current_volume.saturating_sub(1);
                }
            }
        }
    }

    fn sweep_tick(&mut self) {
        let prev_timer = self.sweep_timer;
        self.sweep_timer = self.sweep_timer.saturating_sub(1);

        if prev_timer > 0 && self.sweep_timer == 0 {
            self.sweep_timer = self.sweep_period();
            if self.sweep_enable && self.sweep_period > 0 {
                let new_freq = self.new_freq();
                if new_freq <= 2047 && self.sweep_shift > 0 {
                    self.current_frequency = new_freq;
                    self.frequency = new_freq;
                }
                // recalculate new frequency
                if self.new_freq() > 2047 {
                    self.on = false;
                }
            }
        }
    }

    // return Some(0..=15) value if the channel is enabled, otherwise None
    fn output(&self) -> Option<u8> {
        const WAVEFORM: [[u8; 8]; 4] = [
            [0, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 0, 0, 1],
            [1, 0, 0, 0, 0, 1, 1, 1],
            [0, 1, 1, 1, 1, 1, 1, 0],
        ];

        if !self.on {
            None
        } else {
            Some(WAVEFORM[self.duty as usize][self.phase as usize] * self.current_volume)
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Wave {
    enable: bool,
    length: u16,      // Sound Length = (256-t1)*(1/256) seconds
    output_level: u8, // 0 => mute, 1 => 100%, 2 => 50%, 3 => 25%
    frequency: u16,
    length_enable: bool,
    ram: [u8; 0x10],

    on: bool,
    frequency_timer: u16,
    sample_latch: u8,
    pos: u8,

    length_tick_in: bool,
    prev_length_tick: bool,
}

impl Wave {
    fn reset(&mut self) {
        // Powering APU shouldn't affect wave
        let t = self.ram;
        *self = Default::default();
        self.ram = t;
    }

    // NR30 FF1A E--- ---- DAC power
    // NR31 FF1B LLLL LLLL Length load (256-L)
    // NR32 FF1C -VV- ---- Volume code (00=0%, 01=100%, 10=50%, 11=25%)
    // NR33 FF1D FFFF FFFF Frequency LSB
    // NR34 FF1E TL-- -FFF Trigger, Length enable, Frequency MSB

    fn read(&mut self, regno: usize) -> u8 {
        match regno {
            // NR30: Channel 3 Sound on/off (R/W)
            0 => pack!(7 => self.enable, 0..=6 => !0),
            // NR31: Channel 3 Sound length (R/W)
            1 => !0, // ???
            // NR32: Channel 3 Select output level (R/W)
            2 => pack!(5..=6 => self.output_level, 7 => true, 0..=4 => !0),
            // NR33: Channel 3 Frequency lo (W)
            3 => !0,
            // NR34: Channel 3 Frequency hi (R/W)
            // Only bit 6 can be read
            4 => pack!(6 => self.length_enable, 7 => true, 0..=5 => !0),
            _ => unreachable!(),
        }
    }

    fn write(&mut self, regno: usize, data: u8) {
        match regno {
            // NR30: Channel 3 Sound on/off (R/W)
            0 => self.enable = data.view_bits::<Lsb0>()[7],
            // NR31: Channel 3 Sound length (R/W)
            1 => self.length = 256 - data as u16,
            // NR32: Channel 3 Select output level (R/W)
            2 => self.output_level = data.view_bits::<Lsb0>()[5..=6].load(),
            // NR33: Channel 3 Frequency lo (W)
            3 => self.frequency.view_bits_mut::<Lsb0>()[0..=7].store(data),
            // NR34: Channel 3 Frequency hi (R/W)
            4 => {
                let v = data.view_bits::<Lsb0>();
                self.frequency.view_bits_mut::<Lsb0>()[8..=10].store(v[0..=2].load::<u16>());
                self.length_enable = v[6];
                self.update_tick();
                if v[7] {
                    self.trigger();
                }
            }
            _ => unreachable!(),
        }
        if !self.dac_enable() {
            self.on = false;
        }
        self.update_tick();
    }

    fn dac_enable(&self) -> bool {
        self.enable
    }

    fn trigger(&mut self) {
        self.on = self.dac_enable();
        if self.length == 0 {
            self.length = 256;
        }
        self.frequency_timer = (2048 - self.frequency) * 2;
        self.pos = 0;
    }

    fn tick(&mut self, length_tick: bool) {
        self.frequency_timer = self.frequency_timer.saturating_sub(1);
        if self.frequency_timer == 0 {
            self.frequency_timer = (2048 - self.frequency) * 2;
            self.pos = (self.pos + 1) % 32;
            let v = self.ram[self.pos as usize / 2];
            self.sample_latch = if self.pos % 2 == 0 { v >> 4 } else { v & 0x0F };
        }

        self.length_tick_in = length_tick;
        self.update_tick();
    }

    fn update_tick(&mut self) {
        loop {
            let length_tick = self.length_tick_in && self.length_enable && self.length > 0;
            if self.prev_length_tick == length_tick {
                break;
            }
            if !self.prev_length_tick && length_tick {
                self.length_tick();
            }
            self.prev_length_tick = length_tick;
        }
    }

    fn length_tick(&mut self) {
        if self.length_enable {
            self.length = self.length.saturating_sub(1);
            if self.length == 0 {
                self.on = false;
            }
        }
    }

    fn output(&self) -> Option<u8> {
        if self.on {
            Some(if self.output_level == 0 {
                0
            } else {
                self.sample_latch >> (self.output_level - 1)
            })
        } else {
            None
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
struct Noise {
    length: u8, // Sound Length = (64-t1)*(1/256) seconds
    initial_volume: u8,
    envelope_inc: bool,
    envelope_period: u8, // Length of 1 step = n*(1/64) sec
    clock_shift: u8,
    lsfr_width: bool, // false=15bits, true=7bits
    divisor_code: u8, // Frequency = 524288 Hz / divisor / 2^(clock_shift+1)
    length_enable: bool,

    on: bool,
    current_volume: u8,
    envelope_timer: u8,
    divisor_timer: u8,
    shift_clock_timer: u16,
    lsfr: u16,
    sample_acc: usize,
    sample_count: usize,

    length_tick_in: bool,
    prev_length_tick: bool,
    envelope_tick_in: bool,
    prev_envelope_tick: bool,
}

static DIVISOR: [u8; 8] = [8, 16, 32, 48, 64, 80, 96, 112];

impl Noise {
    fn reset(&mut self) {
        *self = Self::default();
    }

    // Noise
    // FF1F ---- ---- Not used
    // NR41 FF20 --LL LLLL Length load (64-L)
    // NR42 FF21 VVVV APPP Starting volume, Envelope add mode, period
    // NR43 FF22 SSSS WDDD Clock shift, Width mode of LFSR, Divisor code
    // NR44 FF23 TL-- ---- Trigger, Length enable

    fn read(&mut self, regno: usize) -> u8 {
        match regno {
            // NR41: Channel 4 Sound length (R/W)
            0 => !0,
            // NR42: Channel 4 Volume Envelope (R/W)
            1 => pack! {
                4..=7 => self.initial_volume,
                3     => self.envelope_inc,
                0..=2 => self.envelope_period,
            },
            // NR43: Channel 4 Polynomial Counter (R/W)
            2 => pack! {
                4..=7 => self.clock_shift,
                3     => self.lsfr_width,
                0..=2 => self.divisor_code,
            },
            // NR44: Channel 4 Counter/consecutive; initial (R/W)
            // Only bit 6 can be read
            3 => pack!(6 => self.length_enable, 7 => true, 0..=5 => !0),
            _ => unreachable!(),
        }
    }

    fn write(&mut self, regno: usize, data: u8) {
        match regno {
            // NR41: Channel 4 Sound length (R/W)
            0 => self.length = 64 - data.view_bits::<Lsb0>()[0..=5].load::<u8>(),
            // NR42: Channel 4 Volume Envelope (R/W)
            1 => {
                let v = data.view_bits::<Lsb0>();
                self.initial_volume = v[4..=7].load();
                self.envelope_inc = v[3];
                self.envelope_period = v[0..=2].load();
            }
            // NR43: Channel 4 Polynomial Counter (R/W)
            2 => {
                let v = data.view_bits::<Lsb0>();
                self.clock_shift = v[4..=7].load();
                self.lsfr_width = v[3];
                self.divisor_code = v[0..=2].load();
            }
            // NR44: Channel 4 Counter/consecutive; initial (R/W)
            3 => {
                let v = data.view_bits::<Lsb0>();
                self.length_enable = v[6];
                self.update_tick();
                if v[7] {
                    self.trigger();
                }
            }
            _ => unreachable!(),
        }
        if !self.dac_enable() {
            self.on = false;
        }
        self.update_tick();
    }

    fn dac_enable(&self) -> bool {
        self.initial_volume > 0 || self.envelope_inc
    }

    fn trigger(&mut self) {
        self.on = self.dac_enable();
        if self.length == 0 {
            self.length = 64;
        }
        self.current_volume = self.initial_volume;
        self.lsfr = 0x7fff;
        self.divisor_timer = DIVISOR[self.divisor_code as usize] / 2;
        self.shift_clock_timer = 1 << (self.clock_shift + 1);

        log::debug!(
            "NOISE ch trigger: {}Hz, divisor={}, shfit={}",
            524288.0
                / (if self.divisor_code == 0 {
                    0.5
                } else {
                    self.divisor_code as f64
                })
                / 2.0_f64.powi(self.clock_shift as i32 + 1),
            self.divisor_code,
            self.clock_shift,
        );
    }

    fn tick(&mut self, length_tick: bool, envelope_tick: bool) {
        self.shift_clock_timer = self.shift_clock_timer.saturating_sub(1);
        if self.shift_clock_timer == 0 {
            self.shift_clock_timer = 1 << (self.clock_shift + 1);
            self.divisor_timer = self.divisor_timer.saturating_sub(1);
            if self.divisor_timer == 0 {
                self.sample_acc += ((self.lsfr & 1) ^ 1) as usize;
                self.sample_count += 1;

                self.divisor_timer = DIVISOR[self.divisor_code as usize] / 2;
                let b = (self.lsfr & 1) ^ ((self.lsfr >> 1) & 1);
                self.lsfr = if !self.lsfr_width {
                    (self.lsfr >> 1) | (b << 14)
                } else {
                    ((self.lsfr >> 1) & !(1 << 6)) | (b << 6)
                };
            }
        }

        self.length_tick_in = length_tick;
        self.envelope_tick_in = envelope_tick;
        self.update_tick();
    }

    fn update_tick(&mut self) {
        loop {
            let length_tick = self.length_tick_in && self.length_enable && self.length > 0;
            if self.prev_length_tick == length_tick {
                break;
            }

            if !self.prev_length_tick && length_tick {
                self.length_tick();
            }
            self.prev_length_tick = length_tick;
        }

        if !self.prev_envelope_tick && self.envelope_tick_in {
            self.envelope_tick();
        }
        self.prev_envelope_tick = self.envelope_tick_in;
    }

    fn length_tick(&mut self) {
        if self.length_enable {
            self.length = self.length.saturating_sub(1);
            if self.length == 0 {
                self.on = false;
            }
        }
    }

    fn envelope_tick(&mut self) {
        self.envelope_timer = self.envelope_timer.saturating_sub(1);
        if self.envelope_timer == 0 && self.envelope_period > 0 {
            self.envelope_timer = self.envelope_period;

            if self.envelope_inc {
                self.current_volume = min(15, self.current_volume + 1);
            } else {
                self.current_volume = self.current_volume.saturating_sub(1);
            }
        }
    }

    fn output(&mut self) -> Option<u8> {
        if !self.on {
            None
        } else {
            let sample_acc = self.sample_acc + ((self.lsfr & 1) ^ 1) as usize;
            let sample_count = self.sample_count + 1;
            let ret = sample_acc * self.current_volume as usize / sample_count;
            self.sample_acc = 0;
            self.sample_count = 0;
            Some(ret as u8)
        }
    }
}
