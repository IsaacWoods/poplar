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

#[macro_use]
mod vga_buffer;

#[no_mangle]
pub extern fn rust_main() {
    // XXX: The stack is very small and has no guard page!
    vga_buffer::clear_screen();
    println!("Hello, World!");

    loop { }
}

#[lang = "eh_personality"] extern fn eh_personality() { }
#[lang = "panic_fmt"] #[no_mangle] pub extern fn panic_fmt() -> ! { loop {} }
