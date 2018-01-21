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
#![feature(abi_x86_interrupt)]
#![feature(naked_functions)]
#![feature(core_intrinsics)]

/*
 * `rlibc` just provides intrinsics that are linked against, and so the compiler doesn't pick up
 * that it's actually used, so we suppress the warning.
 */
#[allow(unused_extern_crates)] extern crate rlibc;

                extern crate volatile;
                extern crate spin;
                extern crate multiboot2;
#[macro_use]    extern crate bitflags;
                extern crate bit_field;
#[macro_use]    extern crate alloc;
#[macro_use]    extern crate rustos_common;
                extern crate hole_tracking_allocator;

#[macro_use] mod x86_64;
use x86_64 as platform;

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
