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

pub mod gdt;
pub mod tlb;
pub mod tss;

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

pub fn read_msr(msr : u32) -> u64
{
    let (high, low) : (u32, u32);
    unsafe
    {
        asm!("rdmsr" : "={eax}"(low), "={edx}"(high)
                     : "{ecx}"(msr)
                     : "memory"
                     : "volatile");
    }
    ((high as u64) << 32) | (low as u64)
}

pub unsafe fn write_msr(msr : u32, value : u64)
{
    let low  = value as u32;
    let high = (value >> 32) as u32;
    asm!("wrmsr" :: "{ecx}"(msr), "{eax}"(low), "{edx}"(high)
                  : "memory"
                  : "volatile");
}

pub fn init_platform()
{
    assert_first_call!("Must only initialise platform once!");
    const IA32_EFER : u32 = 0xC0000080;

    unsafe
    {
        // Set the NXE bit in the EFER, to allow the use of the No-Execute bit on page tables
        let efer = read_msr(IA32_EFER);
        write_msr(IA32_EFER, efer | (1<<11));

        // Enable write protection
        write_control_reg!(cr0, read_control_reg!(cr0) | (1<<16));
    }
}
