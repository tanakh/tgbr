use anyhow::Result;
use std::path::PathBuf;

#[argopt::cmd]
fn main(
    /// Path to Boot ROM
    #[opt(long)]
    boot_rom: Option<PathBuf>,
    /// Path to Cartridge ROM
    rom_file: Option<PathBuf>,
) -> Result<()> {
    tgbr::app::main(boot_rom, rom_file)
}
