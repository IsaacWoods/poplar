/*
 * Copyright (C) 2016, Philipp Oppermann.
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use header::{Tag, TagIter};

#[repr(packed)]
#[derive(Debug)]
pub struct ModuleTag {
    typ: u32,
    size: u32,
    mod_start: u32,
    mod_end: u32,
    name_byte: u8,
}

impl ModuleTag {
    // The multiboot specification defines the module str
    // as valid utf-8, therefore this function produces
    // defined behavior
    pub fn name(&self) -> &str {
        use core::{mem,str,slice};
        let strlen = self.size as usize - mem::size_of::<ModuleTag>();
        unsafe {
            str::from_utf8_unchecked(
                slice::from_raw_parts(&self.name_byte as *const u8, strlen))
        }
    }

    pub fn start_address(&self) -> u32 {
        self.mod_start
    }

    pub fn end_address(&self) -> u32 {
        self.mod_end
    }
}

pub fn module_iter(iter: TagIter) -> ModuleIter {
    ModuleIter { iter: iter }
}

pub struct ModuleIter {
    iter: TagIter,
}

impl Iterator for ModuleIter {
    type Item = &'static ModuleTag;

    fn next(&mut self) -> Option<&'static ModuleTag> {
        self.iter.find(|x| x.typ == 3)
            .map(|tag| unsafe{&*(tag as *const Tag as *const ModuleTag)})
    }
}
