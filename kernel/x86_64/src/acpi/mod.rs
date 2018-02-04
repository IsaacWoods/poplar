/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

/*
 * The RSDP (Root System Descriptor Pointer) is the first ACPI structure located.
 */
#[derive(Clone,Copy,Debug)]
#[repr(packed)]
pub struct RSDP
{
    signature       : [u8; 8],
    checksum        : u8,
    oem_id          : [u8; 6],
    revision        : u8,
    rsdt_address    : u32,
}
