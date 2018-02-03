# RustOS
An x86_64 OS written in Rust. Currently Multiboot compatible and loaded with GRUB2, but a bootloader
is in the works!

# Roadmap
- [x] Physical memory management
- [x] Virtual memory management / `kalloc` sort of thing
- [x] Exception and interrupt support
- [ ] ACPI framework
- [ ] Custom bootloader and replace `multiboot2` crate (?)
- [ ] Better physical memory allocation (keeping track of freed frames)
- [ ] Better virtual memory allocation
- [ ] Load and execute flat binary program
- [ ] Use APIC & disable PIC
- [ ] Running processes in user mode
- [ ] System calls
- [ ] Scheduling
- [ ] Test harness (maybe using [utest](https://github.com/japaric/utest)) for running unit and regression tests
- [ ] Compile-time memory-map validation
- [ ] Print a stack trace when the kernel panics
