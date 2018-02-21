# Pebble
Pebble is a toy kernel written in Rust for x86_64. It is Multiboot compatible and can be booted by GRUB2.
It is currently very early in development, but future plans are summarised in the roadmap and tracked on the [Trello board](https://trello.com/b/ouyF5ARK/pebble).

# Dependencies
To build Pebble, you will need:
* Nightly `rustc` - tested with `rustc 1.25.0-nightly` (if Pebble fails to build with a later nightly, please open an issue!)
* Xargo - run `cargo install xargo`
* The Rust source code - run `rustup component add rust-src`
* `grub2-mkrescue` - this should already be installed on systems that are booted by GRUB2
* [for running `make run`] `qemu-system-x86_64`
* [for running `make gdb`] [`rust-gdb`](https://github.com/phil-opp/binutils-gdb#gdb-for-64-bit-rust-operating-systems)

# Features / Roadmap
- [x] Kernel support for `alloc` crate
- [x] ACPI framework
- [x] APIC support
- [ ] Userland programs
- [ ] System calls
- [ ] Scheduling
- [x] Logging framework using the `log` crate
- [ ] Test harness (maybe using [utest](https://github.com/japaric/utest))

# Acknowledgements
- [Phil Oppermann's great set of tutorials](https://os.phil-opp.com/)
- The OSDev [wiki](https://wiki.osdev.org/Main_Page) and [forums](https://forum.osdev.org)
- The Rust community at large
