/*
 * Copyright (C) 2018, Pebble Developers.
 * See LICENCE.md
 */

#![feature(lang_items)]
#![feature(asm)]

#![no_std]
#![no_main]

const MESSAGE : &'static str = "Hello from no-std Rust!";

#[no_mangle]
pub extern fn _start() -> !
{
    unsafe
    {
        asm!("mov rdi, 1
              mov rbx, 20
              int 0x80"
             :
             : "rax"(&MESSAGE)
             : "rdi", "rbx"
             : "intel", "volatile");
    }
    loop { }
}

#[lang = "panic_fmt"]
#[no_mangle]
pub extern fn rust_begin_panic(_msg : core::fmt::Arguments, _file : &'static str, _line : u32, _column : u32) -> !
{
    loop { }
}
