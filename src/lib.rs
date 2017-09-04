/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

#![feature(lang_items)]
#![feature(const_fn)]
#![feature(unique)]
#![feature(alloc)]
#![no_std]

extern crate rlibc;
extern crate volatile;
extern crate spin;
extern crate multiboot2;
#[macro_use] extern crate bitflags;
extern crate x86_64;
extern crate bump_allocator;
#[macro_use] extern crate alloc;
#[macro_use] mod util;
#[macro_use] mod vga_buffer;
mod memory;

#[no_mangle]
pub extern fn kmain(multiboot_ptr : usize)
{
    vga_buffer::clear_screen();
    println!("Hello, World!");

    let boot_info = unsafe { multiboot2::load(multiboot_ptr) };
    memory::init(boot_info);

    let mut vec_test = vec![1,2,3,4,5,6,7,8,9];
    vec_test[3] = 42;
    for i in &vec_test
    {
        print!("{} ", i);
    }

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
