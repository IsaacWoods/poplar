/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use ::acpi::RSDP;

#[derive(Clone,Copy,Debug)]
#[repr(packed)]
pub struct RsdpTag
{
    typ             : u32,
    size            : u32,
    rsdp            : RSDP,
}
