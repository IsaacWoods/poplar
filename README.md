# Pebble
Pebble is a toy kernel written in Rust for x86_64. It is Multiboot compatible and can be booted by GRUB2.
It is currently very early in development, but future plans are summarised in the roadmap and tracked on the [Trello board](https://trello.com/b/ouyF5ARK/os).

# Features / Roadmap
- [x] Kernel support for `alloc` crate
- [x] ACPI framework
- [x] APIC support
- [ ] Userland programs
- [ ] System calls
- [ ] Scheduling
- [ ] Test harness (maybe using [utest](https://github.com/japaric/utest))
