/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

/*
 * XXX: Macros have to be defined before they can be used, so define them before the module defs.
 */
macro_rules! read_control_reg
{
    ($reg : ident) =>
    {
        {
            let result : u64;
            unsafe
            {
                asm!(concat!("mov %", stringify!($reg), ", $0") : "=r"(result));
            }
            result
        }
    };
}

/*
 * Because the asm! macro is not wrapped, a call to this macro will need to be inside an unsafe
 * block, which is intended because writing to control registers is probably kinda dangerous.
 */
macro_rules! write_control_reg
{
    ($reg : ident, $value : expr) =>
    {
        asm!(concat!("mov $0, %", stringify!($reg)) :: "r"($value) : "memory");
    };
}

pub mod memory;
pub mod gdt;
pub mod idt;
pub mod tlb;
pub mod tss;
pub mod pic;

#[derive(Copy,Clone,PartialEq,Eq)]
#[repr(u8)]
pub enum PrivilegeLevel
{
    Ring0 = 0,
    Ring1 = 1,
    Ring2 = 2,
    Ring3 = 3,
}

impl From<u16> for PrivilegeLevel
{
    fn from(value : u16) -> Self
    {
        match value
        {
            0 => PrivilegeLevel::Ring0,
            1 => PrivilegeLevel::Ring1,
            2 => PrivilegeLevel::Ring2,
            3 => PrivilegeLevel::Ring3,
            _ => panic!("Invalid privilege level used!"),
        }
    }
}
