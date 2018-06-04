/*
 * Copyright (C) 2016, Philipp Oppermann.
 * Copyright (C) 2017, Pebble Developers.
 * See LICENCE.md
 */

use core::{mem, slice, str};

#[derive(Clone, Copy, Debug)]
#[repr(packed)]
pub struct BootLoaderNameTag {
    typ: u32,
    size: u32,
    string: u8,
}

impl BootLoaderNameTag {
    pub fn name(&self) -> &str {
        unsafe {
            let length = self.size as usize - mem::size_of::<BootLoaderNameTag>();
            str::from_utf8_unchecked(slice::from_raw_parts((&self.string) as *const u8, length))
        }
    }
}
