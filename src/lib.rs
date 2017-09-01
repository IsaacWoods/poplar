/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

#![feature(lang_items)]
#![feature(const_fn)]
#![feature(unique)]
#![no_std]

#![allow(unused_parens)]

extern crate rlibc;
extern crate volatile;
extern crate spin;
extern crate multiboot2;
#[macro_use] extern crate bitflags;
extern crate x86_64;

#[macro_use] mod vga_buffer;
mod memory;

#[no_mangle]
pub extern fn kmain(multiboot_ptr : usize)
{
    // XXX: The stack is very small and has no guard page!
    vga_buffer::clear_screen();
    println!("Hello, World!");

    let boot_info = unsafe { multiboot2::load(multiboot_ptr) };
    let memory_map_tag = boot_info.memory_map_tag().expect("Memory map tag required");
    println!("Memory areas: ");
    for area in memory_map_tag.memory_areas()
    {
        println!("  start: 0x{:x}, length: 0x{:x}", area.base_addr, area.length);
    }

    let elf_sections_tag = boot_info.elf_sections_tag().expect("Elf sections tag required");
    println!("Kernel sections: ");
    for section in elf_sections_tag.sections()
    {
        println!("  addr: 0x{:x}, size: 0x{:x}, flags: 0x{:x}", section.addr, section.size, section.flags);
    }

    let multiboot_start = multiboot_ptr;
    let multiboot_end   = multiboot_start + (boot_info.total_size as usize);
    let kernel_start    = elf_sections_tag.sections().map(|s| s.addr).min().unwrap();
    let kernel_end      = elf_sections_tag.sections().map(|s| s.addr).max().unwrap();
    println!("Multiboot start: 0x{:x}, end: 0x{:x}", multiboot_start, multiboot_end);
    println!("Kernel start: 0x{:x}, end: 0x{:x}", kernel_start, kernel_end);

//    use memory::FrameAllocator;
    let mut frame_allocator = memory::AreaFrameAllocator::new(multiboot_start as usize,
                                                              multiboot_end as usize,
                                                              kernel_start as usize,
                                                              kernel_end as usize,
                                                              memory_map_tag.memory_areas());
    memory::test_paging(&mut frame_allocator);

    loop { }
}

#[lang = "eh_personality"]
extern fn eh_personality() { }

#[lang = "panic_fmt"]
#[no_mangle]
pub extern fn panic_fmt(fmt : core::fmt::Arguments, file : &'static str, line : u32) -> !
{
    println!("\n\nPANIC in {} at line {}:", file, line);
    println!("    {}", fmt);
    loop {}
}
