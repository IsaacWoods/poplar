/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

#![no_std]

#![feature(lang_items)]
#![feature(const_fn)]
#![feature(const_unique_new)]
#![feature(unique)]
#![feature(alloc)]
#![feature(asm)]
#![feature(naked_functions)]
#![feature(core_intrinsics)]

/*
 * `rlibc` just provides intrinsics that are linked against, and so the compiler doesn't pick up
 * that it's actually used, so we suppress the warning.
 */
#[allow(unused_extern_crates)] extern crate rlibc;

                extern crate volatile;
                extern crate spin;
                extern crate bitflags;
                extern crate bit_field;
                extern crate alloc;
                extern crate util;
#[macro_use]    extern crate x86_64 as platform;

mod panic;
pub use panic::panic_fmt;

#[no_mangle]
pub extern fn kmain(multiboot_ptr : usize)
{
    platform::init_platform(multiboot_ptr);

    println!("Hello, World!");
    loop { }
}

#[lang = "eh_personality"] extern fn eh_personality() { }
