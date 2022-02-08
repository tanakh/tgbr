use anyhow::{bail, Result};
use std::{cell::RefCell, path::Path, rc::Rc};

use tgbr::{
    config::{Config, Model},
    gameboy::GameBoy,
    interface::LinkCable,
    rom::Rom,
};

const DMG_BOOT_ROM: &[u8] = include_bytes!("../assets/sameboy-bootroms/dmg_boot.bin");
const MOONEYE_TEST_SUITE_DIR: &str = "tests/mooneye-test-suite/";
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

fn test_mooneye_test_suite(path: impl AsRef<Path>) -> Result<()> {
    let path = Path::new(MOONEYE_TEST_SUITE_DIR).join(path);
    let dat = std::fs::read(path)?;
    let rom = Rom::from_bytes(&dat)?;
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

#[test]
fn test_mooneye_test_suite_roms() -> Result<()> {
    test_mooneye_test_suite("acceptance/add_sp_e_timing.gb")?;
    Ok(())
}
