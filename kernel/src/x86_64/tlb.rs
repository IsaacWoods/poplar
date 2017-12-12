/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use ::memory::VirtualAddress;

pub fn invalidate_page(address : VirtualAddress)
{
    unsafe
    {
        asm!("invlpg ($0)" :: "r"(address) : "memory");
    }
}

pub fn flush()
{
    unsafe
    {
        write_control_reg!(cr3, read_control_reg!(cr3));
    }
}
