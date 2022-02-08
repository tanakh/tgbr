use anyhow::{bail, Result};
use std::{cell::RefCell, rc::Rc};

use tgbr::{
    config::{Config, Model},
    gameboy::GameBoy,
    interface::LinkCable,
    rom::Rom,
};

const DMG_BOOT_ROM: &[u8] = include_bytes!("../assets/sameboy-bootroms/dmg_boot.bin");
const EXPECTED: &[u8] = &[3, 5, 8, 13, 21, 34];

struct TestLinkCable {
    expected: Vec<u8>,
    buf: Vec<u8>,
    completed: Rc<RefCell<Option<bool>>>,
}

impl TestLinkCable {
    fn new(expected: &[u8], completed: &Rc<RefCell<Option<bool>>>) -> Self {
        Self {
            expected: expected.to_vec(),
            buf: vec![],
            completed: Rc::clone(completed),
        }
    }
}

impl LinkCable for TestLinkCable {
    fn send(&mut self, data: u8) {
        self.buf.push(data);
        if self.buf.len() == self.expected.len() {
            *self.completed.borrow_mut() = Some(self.buf == self.expected);
        }
    }

    fn try_recv(&mut self) -> Option<u8> {
        None
    }
}

fn test_mooneye_test_suite(rom_bytes: &[u8]) -> Result<()> {
    let rom = Rom::from_bytes(rom_bytes)?;
    let config = Config::default()
        .set_model(Model::Dmg)
        .set_boot_rom(Some(DMG_BOOT_ROM));

    let mut gb = GameBoy::new(rom, &config)?;

    let completed = Rc::default();
    gb.set_link_cable(Some(TestLinkCable::new(EXPECTED, &completed)));

    let mut frames = 0;
    while completed.borrow().is_none() && frames < 120 {
        gb.exec_frame();
        frames += 1;
    }

    match *completed.borrow() {
        None => bail!("Test timed out"),
        Some(false) => bail!("Test failed"),
        Some(true) => {}
    };

    Ok(())
}

macro_rules! mooneye_test_suite {
    (@process $cur_dir:expr => $test_name:ident, $($rest:tt)*) => {
        mooneye_test_suite!(@process $cur_dir => $test_name => stringify!($test_name), $($rest)*);
    };

    (@process $cur_dir:expr => $test_name:ident => $rom_name:expr, $($rest:tt)*) => {
        #[test]
        #[allow(non_snake_case)]
        fn $test_name() -> anyhow::Result<()> {
            const ROM_BYTES: &[u8] = include_bytes!(concat!($cur_dir, "/", $rom_name, ".gb"));
            test_mooneye_test_suite(ROM_BYTES)
        }
        mooneye_test_suite!(@process $cur_dir => $($rest)*);
    };

    (@process $cur_dir:expr => $dir:ident:: {$($con:tt)*}, $($rest:tt)*) => {
        mod $dir {
            use super::test_mooneye_test_suite;
            mooneye_test_suite!(@process concat!($cur_dir, "/", stringify!($dir)) => $($con)*);
        }
        mooneye_test_suite!(@process $cur_dir => $($rest)*);
    };

    (@process $cur_dir:expr => $(,)?) => {};

    ($($rest:tt)*) => {
        mooneye_test_suite!(@process "mooneye-test-suite/acceptance" => $($rest)*);
    };
}

mooneye_test_suite! {
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
}
