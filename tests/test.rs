use anyhow::{bail, Result};
use std::sync::{Arc, Mutex};

use tgbr::{
    config::{Config, Model},
    gameboy::GameBoy,
    interface::LinkCable,
    rom::Rom,
    BootRoms,
};

fn boot_roms() -> BootRoms {
    BootRoms {
        dmg: Some(include_bytes!("../assets/sameboy-bootroms/dmg_boot.bin").to_vec()),
        cgb: Some(include_bytes!("../assets/sameboy-bootroms/cgb_boot.bin").to_vec()),
        sgb: Some(include_bytes!("../assets/sameboy-bootroms/sgb_boot.bin").to_vec()),
        sgb2: Some(include_bytes!("../assets/sameboy-bootroms/sgb2_boot.bin").to_vec()),
        agb: Some(include_bytes!("../assets/sameboy-bootroms/agb_boot.bin").to_vec()),
    }
}

type Ref<T> = Arc<Mutex<T>>;

trait CheckFn: Fn(&[u8]) -> Option<Result<()>> {}
impl<T> CheckFn for T where T: Fn(&[u8]) -> Option<Result<()>> {}

struct TestLinkCable<F: CheckFn> {
    check_fn: F,
    buf: Ref<Vec<u8>>,
    completed: Ref<Option<Result<()>>>,
}

impl<F: CheckFn> TestLinkCable<F> {
    fn new(check_fn: F, buf: &Ref<Vec<u8>>, completed: &Ref<Option<Result<()>>>) -> Self {
        Self {
            check_fn,
            buf: Ref::clone(buf),
            completed: Ref::clone(completed),
        }
    }
}

impl<F: CheckFn> LinkCable for TestLinkCable<F> {
    fn send(&mut self, data: u8) {
        self.buf.lock().unwrap().push(data);
        *self.completed.lock().unwrap() = (self.check_fn)(self.buf.lock().unwrap().as_slice());
    }

    fn try_recv(&mut self) -> Option<u8> {
        None
    }
}

fn test_serial_output_test_rom(
    rom_bytes: &[u8],
    check_fn: impl CheckFn + Send + Sync + 'static,
) -> Result<()> {
    let boot_roms = boot_roms();
    let rom = Rom::from_bytes(rom_bytes)?;
    let config = Config::default()
        .set_model(Model::Dmg)
        .set_boot_rom(boot_roms);

    let mut gb = GameBoy::new(rom, None, &config)?;

    let buf = Ref::default();
    let completed = Ref::default();
    gb.set_link_cable(Some(TestLinkCable::new(check_fn, &buf, &completed)));

    let mut frames = 0;
    while completed.lock().unwrap().is_none() && frames < 1200 {
        gb.exec_frame(false);
        frames += 1;
    }

    let completed = completed.lock().unwrap();
    match completed.as_ref() {
        None => bail!(
            "Test timed out: output = {}",
            String::from_utf8_lossy(buf.lock().unwrap().as_slice())
        ),
        Some(Ok(())) => Ok(()),
        Some(Err(e)) => bail!("Test failed: {}", e),
    }
}

macro_rules! gen_tester {
    (@process $exp:path, $cur_dir:expr => $test_name:ident, $($rest:tt)*) => {
        gen_tester!(@process $exp, $cur_dir => $test_name => stringify!($test_name), $($rest)*);
    };

    (@process $exp:path, $cur_dir:expr => $test_name:ident => $rom_name:expr, $($rest:tt)*) => {
        #[test]
        #[allow(non_snake_case)]
        fn $test_name() -> anyhow::Result<()> {
            const ROM_BYTES: &[u8] = include_bytes!(concat!($cur_dir, "/", $rom_name, ".gb"));
            test_serial_output_test_rom(ROM_BYTES, $exp)
        }
        gen_tester!(@process $exp, $cur_dir => $($rest)*);
    };

    (@process $exp:path, $cur_dir:expr => $dir:ident:: {$($con:tt)*}, $($rest:tt)*) => {
        mod $dir {
            use super::*;
            gen_tester!(@process $exp, concat!($cur_dir, "/", stringify!($dir)) => $($con)*);
        }
        gen_tester!(@process $exp, $cur_dir => $($rest)*);
    };

    (@process $exp:path, $cur_dir:expr => $(,)?) => {};

    ($tag:ident, $path:literal, $exp:path: {$($con:tt)*}, $($rest:tt)*) => {
        mod $tag {
            #[allow(unused)]
            use super::*;
            gen_tester!(@process $exp, $path => $($con)*);
        }
        gen_tester!($($rest)*);
    };
    () => {};
}

fn blargg_check_fn(output: &[u8]) -> Option<Result<()>> {
    const EXPECTED: &[u8] = b"Passed\n";
    const FAILED: &[u8] = b"FAILED\n";

    if output.len() >= EXPECTED.len() && &output[output.len() - EXPECTED.len()..] == EXPECTED {
        return Some(Ok(()));
    }

    if output.len() >= FAILED.len() && &output[output.len() - FAILED.len()..] == FAILED {
        return Some(Err(anyhow::anyhow!(
            "Test failed: {}",
            String::from_utf8_lossy(output)
        )));
    }

    None
}

fn mooneye_check_fn(output: &[u8]) -> Option<Result<()>> {
    const EXPECTED: &[u8] = &[3, 5, 8, 13, 21, 34];

    if output.len() == EXPECTED.len() {
        Some(if output == EXPECTED {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Output did not match expected: {:?}",
                output
            ))
        })
    } else {
        None
    }
}

gen_tester! {
    blargg, "blargg", blargg_check_fn: {
        cpu_instrs::{
            // cpu_instrs,
            individual::{
                _01_special => "01-special",
                _02_interrupts => "02-interrupts",
                _03_op_sp_hl => "03-op sp,hl",
                _04_op_r_imm => "04-op r,imm",
                _05_op_rp => "05-op rp",
                _06_ld_r_r => "06-ld r,r",
                _07_jr_jp_call_ret_rst => "07-jr,jp,call,ret,rst",
                _08_misc_instrs => "08-misc instrs",
                _09_op_r_r => "09-op r,r",
                _10_bit_ops => "10-bit ops",
                _11_op_a_hl => "11-op a,(hl)",
            },
        },
        mem_timing::{
            individual::{
                _01_read_timing => "01-read_timing",
                _02_write_timing => "02-write_timing",
                _03_modify_timing => "03-modify_timing",
            },
        },
        // dmg_sound::{
        //     rom_singles::{
        //         _01_registers => "01-registers",
        //         _02_len_ctr => "02-len ctr",
        //         _03_trigger => "03-trigger",
        //         _04_sweep => "04-sweep",
        //         _05_sweep_details => "05-sweep details",
        //         _06_overflow_on_trigger => "06-overflow on trigger",
        //         _07_len_sweep_period_sync => "07-len sweep period sync",
        //         _08_len_ctr_during_power => "08-len ctr during power",
        //         _09_wave_read_while_on => "09-wave read while on",
        //         _10_wave_trigger_while_on => "10-wave trigger while on",
        //         _11_regs_after_power => "11-regs after power",
        //         _12_wave_write_while_on => "12-wave write while on",
        //     },
        // },
        // halt_bug,
        // instr_timing::{
        //     instr_timing,
        // },
        // interrupt_time::{
        //     interrupt_time,
        // },
        // oam_bug::{
        //     rom_singles::{
        //         _1_lcd_sync => "1-lcd_sync",
        //         _2_causes => "2-causes",
        //         _3_non_causes => "3-non_causes",
        //         _4_scanline_timing => "4-scanline_timing",
        //         _5_timing_bug => "5-timing_bug",
        //         _6_timing_no_bug => "6-timing_no_bug",
        //         _7_timing_effect => "7-timing_effect",
        //         _8_instr_effect => "8-instr_effect",
        //     },
        // },
    },

    mooneye, "mooneye-test-suite/acceptance", mooneye_check_fn: {
        add_sp_e_timing,
        // boot_div-S,
        // boot_div-dmg0,
        boot_div_dmgABCmgb => "boot_div-dmgABCmgb",
        // boot_div2-S,
        // boot_hwio-S,
        // boot_hwio-dmg0,
        boot_hwio_dmgABCmgb => "boot_hwio-dmgABCmgb",
        // boot_regs-dmg0,
        boot_regs_dmgABC => "boot_regs-dmgABC",
        // boot_regs-mgb,
        // boot_regs-sgb,
        // boot_regs-sgb2,
        call_cc_timing,
        call_cc_timing2,
        call_timing,
        call_timing2,
        di_timing_GS => "di_timing-GS",
        div_timing,
        ei_sequence,
        ei_timing,
        halt_ime0_ei,
        halt_ime0_nointr_timing,
        halt_ime1_timing,
        halt_ime1_timing2_GS => "halt_ime1_timing2-GS",
        if_ie_registers,
        intr_timing,
        jp_cc_timing,
        jp_timing,
        ld_hl_sp_e_timing,
        oam_dma_restart,
        oam_dma_start,
        oam_dma_timing,
        pop_timing,
        push_timing,
        rapid_di_ei,
        ret_cc_timing,
        ret_timing,
        reti_intr_timing,
        reti_timing,
        rst_timing,

        bits::{
            mem_oam,
            reg_f,
            unused_hwio_GS => "unused_hwio-GS",
        },
        instr::{
            daa,
        },
        interrupts::{
            ie_push,
        },
        oam_dma::{
            basic,
            reg_read,
            sources_GS => "sources-GS",
        },
        ppu::{
            hblank_ly_scx_timing_GS => "hblank_ly_scx_timing-GS",
            intr_1_2_timing_GS => "intr_1_2_timing-GS",
            intr_2_0_timing,
            intr_2_mode0_timing,
            intr_2_mode0_timing_sprites,
            intr_2_mode3_timing,
            intr_2_oam_ok_timing,
            lcdon_timing_GS => "lcdon_timing-GS",
            lcdon_write_timing_GS => "lcdon_write_timing-GS",
            stat_irq_blocking,
            stat_lyc_onoff,
            vblank_stat_intr_GS => "vblank_stat_intr-GS",
        },
        serial::{
            boot_sclk_align_dmgABCmgb => "boot_sclk_align-dmgABCmgb",
        },
        timer::{
            div_write,
            rapid_toggle,
            tim00,
            tim00_div_trigger,
            tim01,
            tim01_div_trigger,
            tim10,
            tim10_div_trigger,
            tim11,
            tim11_div_trigger,
            tima_reload,
            tima_write_reloading,
            tma_write_reloading,
        },
    },
}
