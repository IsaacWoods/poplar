/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

#![no_std]

#![feature(lang_items)]
#![feature(const_fn)]
#![feature(unique)]
#![feature(alloc)]
#![feature(abi_x86_interrupt)]

/*
 * The compiler sometimes doesn't pick up on crates being used, so we have to supress a few
 * warnings.
 */
#[allow(unused_extern_crates)] extern crate rlibc;
                               extern crate volatile;
                               extern crate spin;
#[macro_use]                   extern crate lazy_static;
                               extern crate multiboot2;
#[macro_use]                   extern crate bitflags;
                               extern crate bit_field;
                               extern crate x86_64;
#[macro_use]                   extern crate alloc;
#[macro_use]                   extern crate rustos_common;
                               extern crate hole_tracking_allocator;

#[macro_use]                   mod vga_buffer;
                               mod memory;
                               mod interrupts;

#[no_mangle]
pub extern fn kmain(multiboot_ptr : usize)
{
    vga_buffer::clear_screen();

    let boot_info = unsafe { multiboot2::load(multiboot_ptr) };
    let mut memory_controller = memory::init(boot_info);
    interrupts::init(&mut memory_controller);

    unsafe
    {
        *(0xdeadbeef as *mut u64) = 42;
    }

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
