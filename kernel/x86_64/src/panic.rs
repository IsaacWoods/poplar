/*
 * Copyright (C) 2017, Pebble Developers.
 * See LICENCE.md
 */

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
