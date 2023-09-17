# Poplar
[![Build](https://github.com/IsaacWoods/poplar/actions/workflows/build.yml/badge.svg)](https://github.com/IsaacWoods/poplar/actions/workflows/build.yml)
[![License: MPL-2.0](https://img.shields.io/badge/license-MPL--2.0-blue.svg)](https://opensource.org/licenses/MPL-2.0)

Poplar is a microkernel and userspace written in Rust, exploring modern ideas. It is not a UNIX, and does not aim
for compatibility with existing software. It currently supports x86_64 and RISC-V.

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

### Building
This repository includes an [`xtask`-based](https://github.com/matklad/cargo-xtask) build tool to simplify building and running Poplar.
The tool can be configured in `Poplar.toml` - this can, for example,  be used to set whether to build using the
release profile, and the architecture to build for.

* Running `cargo xtask dist` will build a disk image
* Running `cargo xtask qemu` will build a disk image, and then start emulating it in QEMU

See `cargo xtask --help` for more information about how to invoke the build system.
