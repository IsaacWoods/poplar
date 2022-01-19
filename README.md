# Poplar
![Build status](https://github.com/IsaacWoods/poplar/actions/workflows/build/badge.svg)
[![License: MPL-2.0](https://img.shields.io/badge/license-MPL--2.0-blue.svg)](https://opensource.org/licenses/MPL-2.0)

**Poplar was previously called Pebble. It was renamed to avoid confusion with the OS that runs on the [defunct
Pebble smartwatches](https://en.wikipedia.org/wiki/Poplar_(watch))**

Poplar is a microkernel and userspace written in Rust, exploring modern ideas. It is not a UNIX, and does not aim
for compatibility with existing software.

The best way to learn about Poplar is to read [the book](https://isaacwoods.github.io/poplar/book/).
[The website](https://isaacwoods.github.io/poplar) also hosts some other useful resources.

## Building and running
**Operating systems tend to be complex to build and run. We've tried to make this as simple as we can, but if you
encounter problems or have suggestions to make it easier, feel free to file an issue :)**

### Getting the source
Firstly, clone the repository and fetch the submodules:
```
git clone https://github.com/IsaacWoods/poplar.git
git submodule update --init --recursive
```

### Things you'll need
- A nightly Rust toolchain
- The `rust-src` component (install with `rustup component add rust-src`)
- A working QEMU installation (one that provides `qemu-system-{arch}`)

To compile userspace programs, you'll need to build our custom Rust toolchain:
- Clone [`IsaacWoods/rust`](https://github.com/IsaacWoods/rust/tree/poplar) and checkout the `poplar` branch
- (Optional) rebase against `rust-lang/rust` to get the latest chages
- Copy `isaacs_config.toml` to `config.toml` (or use your own)
- Run `./x.py build -i library/std` to build a stage-1 compiler and `libstd`
- Create a toolchain with `rustup toolchain link poplar build/{host triple}/stage1` (e.g. `rustup toolchain link poplar build/x86_64-unknown-linux-gnu/stage1`)

**You don't need this toolchain to build the bootloaders, kernel, or `no_std` user programs, so you can get started
without it!**

### Building
This repository includes an [`xtask`-based](https://github.com/matklad/cargo-xtask) build tool to simplify building and running Poplar.

* Running `cargo xtask dist` will build a disk image for x86_64
* Running `cargo xtask qemu` will build a disk image for x86_64, and then start emulating it into QEMU

See `cargo xtask --help` for more information about how to invoke the build system.
