# TGB-R

TGB-R is an open source Game Boy and Game Boy Color emulator.

## Install

### Pre-build binary

Download pre-build binary archive from [Releases Page](https://github.com/tanakh/tgbr/releases) and extract it to an appropriate directory.

### Build from source

First, [install the Rust toolchain](https://www.rust-lang.org/tools/install) so that you can use the `cargo` commands.

You can use the `cargo` command to build and install it from the source code.

```sh
$ cargo install tgbr
```

To use the development version, please clone this repository.

```sh
$ git clone https://github.com/tanakh/tgbr
$ cd tgbr
$ cargo run --release
```

## Usage

Execute `tgbr.exe` or `tgbr` and load ROM from GUI.

By default, the Esc key returns to the menu. The hotkeys can be changed from the hotkey settings in the menu.

## Features

* Game Boy emulation
* Game Boy Color emulation
* State save/load
* Turbo speed
* Rewind like Nintendo Switch's nes/snes emulator.
* Builtin [SameBoy's open source boot ROM](https://github.com/LIJI32/SameBoy/tree/master/BootROMs) for accuracy and configuring colors to non-GBC title.

## License

[MIT](LICENSE)
