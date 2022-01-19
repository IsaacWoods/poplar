/*
 * NOTE: This assumes that a byte is 8 bits long. I don't think I'll ever be insane enough to
 * cater for an architecture where this isn't true, so I'm gonna call this platform-independent.
 */

use core::{fmt, mem};
use num_traits::PrimInt;

/// Values can be wrapped in this type when they're printed to display them as easy-to-read binary
/// numbers. `Display` is implemented to print the value in the form `00000000-00000000`, while
/// `Debug` will print it in the form `00000000(8)-00000000(0)` (with offsets of each byte).
pub struct BinaryPrettyPrint<T: fmt::Binary + PrimInt>(pub T);

impl<T: fmt::Binary + PrimInt> fmt::Display for BinaryPrettyPrint<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let byte_mask: T = T::from(0xff).unwrap();
        let max_byte = mem::size_of::<T>() - 1;

        for i in 0..max_byte {
            let byte = max_byte - i;
            write!(f, "{:>08b}-", (self.0 >> (byte * 8)) & byte_mask).unwrap();
        }
        write!(f, "{:>08b}", self.0 & byte_mask).unwrap();

        Ok(())
    }
}

impl<T: fmt::Binary + PrimInt> fmt::Debug for BinaryPrettyPrint<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let byte_mask: T = T::from(0xff).unwrap();
        let max_byte = mem::size_of::<T>() - 1;

        for i in 0..max_byte {
            let byte = max_byte - i;
            write!(f, "{:>08b}({})-", (self.0 >> (byte * 8)) & byte_mask, byte * 8).unwrap();
        }
        write!(f, "{:>08b}(0)", self.0 & byte_mask).unwrap();

        Ok(())
    }
}

#[test]
fn test() {
    assert_eq!(format!("{}", BinaryPrettyPrint(0 as u8)), "00000000");
    assert_eq!(format!("{}", BinaryPrettyPrint(0 as u16)), "00000000-00000000");
    assert_eq!(format!("{}", BinaryPrettyPrint(0 as u32)), "00000000-00000000-00000000-00000000");
}
