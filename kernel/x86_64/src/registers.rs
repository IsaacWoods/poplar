/*
 * Copyright (C) 2017, Pebble Developers.
 * See LICENCE.md
 */

use core::fmt;

#[derive(Clone,Copy)]
pub struct CpuFlags(pub u64);

impl fmt::Debug for CpuFlags
{
    fn fmt(&self, f : &mut fmt::Formatter) -> fmt::Result
    {
        write!(f, "[{}{}{}{}{}{}{}{}{}{}{}] {:#x}",
                  if self.0 & 0x0000_4000 > 0 { 'N' } else { '-' },         // Nested Task flag
                  ['0','1','2','3'][(self.0 & 0x0000_3001 >> 12) as usize], // I/O privilege level
                  if self.0 & 0x0000_0800 > 0 { 'O' } else { '-' },         // Overflow flag
                  if self.0 & 0x0000_0400 > 0 { 'D' } else { '-' },         // Direction flag
                  if self.0 & 0x0000_0200 > 0 { 'I' } else { '-' },         // Interrupt flag
                  if self.0 & 0x0000_0100 > 0 { 'T' } else { '-' },         // Trap flag
                  if self.0 & 0x0000_0080 > 0 { 'S' } else { '-' },         // Sign flag
                  if self.0 & 0x0000_0040 > 0 { 'Z' } else { '-' },         // Zero flag
                  if self.0 & 0x0000_0010 > 0 { 'A' } else { '-' },         // Adjust flag
                  if self.0 & 0x0000_0004 > 0 { 'P' } else { '-' },         // Parity flag
                  if self.0 & 0x0000_0001 > 0 { 'C' } else { '-' },         // Carry flag
                  self.0)
    }
}

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
