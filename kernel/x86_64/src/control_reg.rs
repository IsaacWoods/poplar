/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
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
