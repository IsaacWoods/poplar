/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

//! It's useful to be able to model an integral type such as `u32` as being a series of bits,
//! instead of a whole number. There are, of course, the usual bitwise operators for simple stuff,
//! but the `Bitmap` trait provides more complex, specific operations that are useful for bitmaps.
//!
//! A common use of the `Bitmap` trait is for memory allocators to track an area of pages, where
//! each bit represents a page. You might, for example, want to find a series of `n` zeros (which
//! would mark an area of `n` pages that are free to allocate) - the `alloc` method provides this
//! functionality.

use bit_field::{BitArray, BitField};
use core::{fmt::Debug, mem};
use num_traits::PrimInt;

pub trait Bitmap: Sized {
    /// Find `n` consecutive unset bits, set them and return the index of the first bit.
    fn alloc(&mut self, n: usize) -> Option<usize>;

    /// Free `n` previously allocated bits, starting at `index`.
    fn free(&mut self, index: usize, n: usize);
}

impl<T> Bitmap for T
where
    T: PrimInt + BitField + Debug,
{
    fn alloc(&mut self, n: usize) -> Option<usize> {
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
                return Some(i);
            }
        }

        None
    }

    fn free(&mut self, index: usize, n: usize) {
        assert_eq!(self.get_bits(index..(index + n)), Self::from((Self::one() << n) - Self::one()).unwrap());
        self.set_bits(index..(index + n), Self::zero());
    }
}

/// Like `Bitmap`, but for arrays. This is unfortunately needed due to conflicting implementations.
pub trait BitmapSlice: Sized {
    /// Find `n` consecutive unset bits, set them and return the index of the first bit.
    /// This is useful for memory managers using `Bitmap` to track free frames / pages.
    fn alloc(self, n: usize) -> Option<usize>;

    /// Free `n` previously allocated bits, starting at `index`.
    fn free(self, index: usize, n: usize);
}

impl BitmapSlice for &mut [u8] {
    fn alloc(self, n: usize) -> Option<usize> {
        let num_bits = 8 * self.len();
        let mask = (1 << n) - 1;

        for i in 0..(num_bits - n) {
            if self.get_bits(i..(i + n)) & mask == 0 {
                self.set_bits(i..(i + n), mask);
                return Some(i);
            }
        }

        None
    }

    fn free(self, index: usize, n: usize) {
        assert_eq!(self.get_bits(index..(index + n)), (1 << n) - 1);
        self.set_bits(index..(index + n), 0);
    }
}

#[test]
fn test_bitmap() {
    assert_eq!((0b10001: u16).alloc(3), Some(1));
    assert_eq!((0b11_0000_1_000_111: u16).alloc(4), Some(7));
    assert_eq!((0b1111_1111: u8).alloc(1), None);
    assert_eq!((0b0110_1010: u8).alloc(2), None);
}

#[test]
fn test_bitmap_array_simple() {
    /*
     * These might be a bit counterintuitive at first, because `BitArray` treats the array as a
     * little-endian set of bytes, so the LSB of the first byte is bit 0.
     */
    assert_eq!([0xff, 0xff, 0xff, 0xff, 0xff].alloc(3), None);
    assert_eq!([0xfe, 0xff, 0xff, 0xff].alloc(1), Some(0));
    assert_eq!([0b1111_0001, 0xff, 0xff].alloc(3), Some(1));
    assert_eq!([0b1111_1001, 0xff, 0xff].alloc(3), None);
}

#[test]
fn test_bitmap_array_multiple() {
    let mut bitmap = [0b1111_1100, 0xff, 0xff];

    // Make the first allocation
    let first = bitmap.alloc(1);
    assert_eq!(first, Some(0));
    assert_eq!(bitmap, [0b1111_1101, 0xff, 0xff]);

    // Make a second allocation
    let second = bitmap.alloc(1);
    assert_eq!(second, Some(1));
    assert_eq!(bitmap, [0b1111_1111, 0xff, 0xff]);

    // Try to make a third allocation - it should fail
    assert_eq!(bitmap.alloc(1), None);
}

#[test]
fn test_bitmap_free() {
    let mut bitmap = [0b1000_1000, 0xff, 0xff];

    // Make the first allocation
    let first = bitmap.alloc(2);
    assert_eq!(first, Some(0));
    assert_eq!(bitmap, [0b1000_1011, 0xff, 0xff]);

    // Make a second allocation
    let second = bitmap.alloc(3);
    assert_eq!(second, Some(4));
    assert_eq!(bitmap, [0b1111_1011, 0xff, 0xff]);

    // Make an allocation that doesn't fit
    assert_eq!(bitmap.alloc(3), None);

    // Free the first allocation
    bitmap.free(first.unwrap(), 2);
    assert_eq!(bitmap, [0b1111_1000, 0xff, 0xff]);

    // Make another allocation that now fits
    let third = bitmap.alloc(3);
    assert_eq!(third, Some(0));
    assert_eq!(bitmap, [0b1111_1111, 0xff, 0xff]);
}
