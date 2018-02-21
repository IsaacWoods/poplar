/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

#[lang = "panic_fmt"]
#[no_mangle]
pub extern fn panic_fmt(fmt     : ::core::fmt::Arguments,
                        file    : &'static str,
                        line    : u32) -> !
{
    serial_println!("\n\nPANIC in {} at line {}:", file, line);
    serial_println!("      {}", fmt);

    println!("\n\nPANIC in {} at line {}:", file, line);
    println!("      {}", fmt);
    loop {}
}
