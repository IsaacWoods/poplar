/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use ::memory::paging::PhysicalAddress;

#[derive(Clone,Copy,Debug)]
#[repr(packed)]
pub struct FramebufferTag
{
    typ                     : u32,
    size                    : u32,
    pub address             : PhysicalAddress,
    pub pitch               : u32,
    pub width               : u32,
    pub height              : u32,
    pub bits_per_pixel      : u8,
    pub framebuffer_type    : u8,   // XXX: This must be 1 for this structure to be valid
    reserved_1              : u8,

    // Color info
    red_field_position      : u8,
    red_mask_size           : u8,
    green_field_position    : u8,
    green_mask_size         : u8,
    blue_field_position     : u8,
    blue_mask_size          : u8,
}
