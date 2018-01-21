/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use super::memory::VirtualAddress;

/*
 * Hardware task switching isn't supported on x86_64, but we still have the TSS structure. It's
 * used to store kernel-level stacks that should be used if interrupts occur (this is used to
 * prevent triple-faults from occuring if we overflow the kernel stack).
 */

#[derive(Clone,Copy,Debug)]
#[repr(C,packed)]
pub struct Tss
{
    reserved_1                  : u32,
    pub privilege_stack_table   : [VirtualAddress; 3],
    reserved_2                  : u64,
    pub interrupt_stack_table   : [VirtualAddress; 7],
    reserved_3                  : u64,
    reserved_4                  : u16,
    pub iomap_base              : u16,
}

impl Tss
{
    pub const fn new() -> Tss
    {
        Tss
        {
            reserved_1              : 0,
            privilege_stack_table   : [0; 3],
            reserved_2              : 0,
            interrupt_stack_table   : [0; 7],
            reserved_3              : 0,
            reserved_4              : 0,
            iomap_base              : 0,
        }
    }
}
