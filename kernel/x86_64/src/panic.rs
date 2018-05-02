/*
 * Copyright (C) 2017, Pebble Developers.
 * See LICENCE.md
 */

use cpu;

#[lang = "eh_personality"]
#[no_mangle]
pub extern "C" fn rust_eh_personality()
{
}

#[lang = "panic_fmt"]
#[no_mangle]
pub extern fn panic_fmt(fmt     : ::core::fmt::Arguments,
                        file    : &'static str,
                        line    : u32) -> !
{
    error!("PANIC in {} at line {}: \n    {}", file, line, fmt);

    #[allow(empty_loop)]
    loop {}
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern fn _Unwind_Resume()
{
    loop
    {
        unsafe
        {
            cpu::halt();
        }
    }
}
