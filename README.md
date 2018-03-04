# The Pebble Microkernel
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![](https://tokei.rs/b1/github/pebble-os/kernel)](https://github.com/Aaronepower/tokei)

This is the Pebble microkernel. It is written in Rust and currently only supports x86_64.
It is Multiboot2 compatible and can be booted by GRUB2.

# Dependencies
To build the microkernel, you will need:
* Nightly `rustc` - tested with `rustc 1.26.0-nightly` (if Pebble fails to build with a later nightly, please open an issue!)
* Xargo - run `cargo install xargo`
* The Rust source code - run `rustup component add rust-src`
* `grub2-mkrescue` - this should already be installed on systems that are booted by GRUB2
* [for `make run`] `qemu-system-x86_64`
* [for `make gdb`] [`rust-gdb`](https://github.com/phil-opp/binutils-gdb#gdb-for-64-bit-rust-operating-systems)

# Bird's-eye view
The kernel is made up of a few crates, centered around the `kernel` crate:
```

                           kernel
                          /  ▲
                         /   |
                        /    |
                       ▼     |
                      log    |
                       ▲     |
                        \    |
                         \   |
                    {architecture crate}
                        * x86_64

```

* The `kernel` crate contains platform-independent kernel code and manages the overall control of the kernel.
It also provides the kernel interface to userland programs and services.
* The "architecture crate" (e.g. `x86_64`) contains platform-specific kernel code, including the entry to the kernel and memory management code.
It initialises the platform, then passes control to the `kernel` crate.
* `log` is used for logging across all kernel crates. The actual logger is created and maintained by the architecture crate.

This entire crate heirachy is compiled into a static library from the architecture crate, and then linked against other kernel objects (depending on platform).
This modularity is meant to make it as easy as possible to extend the kernel to other architectures in the future.

# Features / Roadmap
- [x] `alloc` support
- [x] ACPI framework
- [x] APIC support
- [ ] Userland programs
- [ ] System calls
- [ ] Scheduling
- [x] Logging framework using the `log` crate
- [ ] Test harness (maybe using [utest](https://github.com/japaric/utest))
