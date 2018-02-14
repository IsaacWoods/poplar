# RustOS
An x86_64 OS written in Rust. Currently Multiboot compatible and loaded with GRUB2.
Current tasks and plans for the future are summarised in the Roadmap and [on the Trello board](https://trello.com/b/ouyF5ARK/os).

# Roadmap
- [x] Physical memory management
- [x] Virtual memory management / `kalloc` sort of thing
- [x] Exception and interrupt support
- [.] ACPI framework
- [x] Load and execute flat binary program
- [ ] Better physical memory allocation (keeping track of freed frames)
- [ ] Better virtual memory allocation
- [ ] Use APIC & disable PIC
- [ ] Running processes in user mode
- [ ] System calls
- [ ] Scheduling
- [ ] Test harness (maybe using [utest](https://github.com/japaric/utest)) for running unit and regression tests
- [ ] Compile-time memory-map validation
- [ ] Print a stack trace when the kernel panics
- [ ] Custom bootloader and replace `multiboot2` crate (?)
