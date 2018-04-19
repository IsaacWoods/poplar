# The Microkernel
This is the Pebble microkernel. It is written in Rust and currently only supports x86_64.
It is Multiboot2 compatible and can be booted by GRUB2.

## Features / Roadmap
- [x] `alloc` support
- [x] ACPI framework
- [ ] AML interpreter
- [x] APIC support
- [ ] Userland programs
- [ ] System calls
- [ ] Scheduling
- [x] Logging framework using the `log` crate
- [ ] Test harness (maybe using [utest](https://github.com/japaric/utest))
