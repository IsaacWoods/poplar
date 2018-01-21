/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use super::memory::VirtualAddress;

pub fn invalidate_page(address : VirtualAddress)
{
    unsafe
    {
        asm!("invlpg ($0)" :: "r"(address) : "memory");
    }
}

pub fn flush()
{
    let current_cr3 = read_control_reg!(cr3);
    unsafe
    {
        write_control_reg!(cr3, current_cr3);
    }
}
