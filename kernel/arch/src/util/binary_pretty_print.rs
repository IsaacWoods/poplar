/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use num_traits::PrimInt;
use core::{mem,fmt};

pub struct BinaryPrettyPrint<T : fmt::Binary + PrimInt>(pub T);

impl<T : fmt::Binary + PrimInt> fmt::Display for BinaryPrettyPrint<T>
{
    fn fmt(&self, f : &mut fmt::Formatter) -> fmt::Result
    {
        let byte_mask : T = T::from(0xff).unwrap();
        let max_byte = mem::size_of::<T>() - 1;

        for i in 0..max_byte
        {
            let byte = max_byte - i;
            write!(f, "{:>08b}-", (self.0 >> (byte * 8)) & byte_mask);
        }
        write!(f, "{:>08b}", self.0 & byte_mask);

        Ok(())
    }
}

#[test]
fn test()
{
    assert_eq!(format!("{}", BinaryPrettyPrint(0 as u8)),   "00000000");
    assert_eq!(format!("{}", BinaryPrettyPrint(0 as u16)),  "00000000-00000000");
    assert_eq!(format!("{}", BinaryPrettyPrint(0 as u32)),  "00000000-00000000-00000000-00000000");
}
