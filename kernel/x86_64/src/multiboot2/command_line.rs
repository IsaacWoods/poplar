/*
 * Copyright (C) 2016, Philipp Oppermann.
 * Copyright (C) 2017, Pebble Developers.
 * See LICENCE.md
 */

use core::{mem,str,slice};

#[derive(Clone,Copy,Debug)]
#[repr(packed)] // repr(C) would add unwanted padding before first_section
pub struct CommandLineTag
{
    typ     : u32,
    size    : u32,
    string  : u8,
}

impl CommandLineTag
{
    pub fn command_line(&self) -> &str
    {
        unsafe
        {
            let length = self.size as usize - mem::size_of::<CommandLineTag>();
            str::from_utf8_unchecked(slice::from_raw_parts((&self.string) as *const u8, length))
        }
    }
}
