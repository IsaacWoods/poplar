/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use ::acpi::Rsdp;

#[derive(Clone,Copy,Debug)]
#[repr(packed)]
pub struct RsdpTag
{
    typ             : u32,
    size            : u32,
    rsdp            : Rsdp,
}

impl RsdpTag
{
    pub fn rsdp(&'static self) -> &'static Rsdp
    {
        &self.rsdp
    }
}
