# RustOS
An x86_64 OS written in Rust. Currently Multiboot compatible and loaded with GRUB2, but a bootloader
is in the works!

# Features / Roadmap
- [x] Physical memory management
- [x] Virtual memory management / `kalloc`-esque
- [x] Exception and interrupt support
- [ ] ACPI framework
- [ ] Custom bootloader
- [ ] Replace `multiboot2` crate for custom bootloader
- [ ] Better virtual memory allocation
- [ ] Load and execute flat binary program
- [ ] Use APIC & disable PIC
- [ ] Running processes in user mode
- [ ] System calls
