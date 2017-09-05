/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

#![no_std]

#![feature(lang_items)]
#![feature(const_fn)]
#![feature(unique)]
#![feature(alloc)]

#[allow(unused_extern_crates)] extern crate rlibc;  // This is used, but the compiler doesn't realise it
extern crate volatile;
extern crate spin;
extern crate multiboot2;
#[macro_use] extern crate bitflags;
extern crate x86_64;
#[macro_use] extern crate alloc;
#[macro_use] extern crate rustos_common;
extern crate hole_tracking_allocator;

#[macro_use] mod vga_buffer;
mod memory;

#[no_mangle]
pub extern fn kmain(multiboot_ptr : usize)
{
    vga_buffer::clear_screen();

    let boot_info = unsafe { multiboot2::load(multiboot_ptr) };
    memory::init(boot_info);

    println!("Hello, World!");

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
