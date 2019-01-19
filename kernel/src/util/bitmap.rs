//! It's useful to be able to model an integral type such as `u32` as being a series of bits,
//! instead of a whole number. There are, of course, the usual bitwise operators for simple stuff,
//! but the `Bitmap` trait provides more complex, specific operations that are useful for bitmaps.
//!
//! A common use of the `Bitmap` trait is for memory allocators to track an area of pages, where
//! each bit represents a page. You might, for example, want to find a series of `n` zeros (which
//! would mark an area of `n` pages that are free to allocate) - the `alloc_n` method provides this
//! functionality.

use bit_field::BitField;
use core::mem;
use num::PrimInt;

pub trait Bitmap: Sized {
    /// Find `n` consecutive unset bits, set them and return the index of the first bit.
    /// This is useful for memory managers using `Bitmap` to track free frames / pages.
    fn alloc_n(&mut self, n: usize) -> Option<Self>;
}

impl<T> Bitmap for T
where
    T: PrimInt + BitField,
{
    fn alloc_n(&mut self, n: usize) -> Option<T> {
        let num_bits = 8 * mem::size_of::<Self>();
        let mask = Self::from((Self::one() << n) - Self::one()).unwrap();

        /*
         * For each bit before there are no longer `n` bits to the end, take the next `n` bits
         * and and them with a mask of `n` ones. If the result is zero, all the bits in
         * the slice must be 0 and so we've found a run of `n` zeros.
         */
        for i in 0..(num_bits - n) {
            if self.get_bits(i..(i + n)) & mask == Self::zero() {
                self.set_bits(i..(i + n), mask);
                return Some(Self::from(i).unwrap());
            }
        }

        None
    }
}

#[test]
fn test_bitmap_alloc_n() {
    assert_eq!((0b10001: u16).alloc_n(3), Some(1));
    assert_eq!((0b11_0000_1_000_111: u16).alloc_n(4), Some(7));
    assert_eq!((0b1111_1111: u8).alloc_n(1), None);
    assert_eq!((0b0110_1010: u8).alloc_n(2), None);
}
