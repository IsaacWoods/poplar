//! This is an in-tree copy of parts of the `bit_field` crate, which provides a useful trait for getting and setting bit
//! ranges on primitive types. We use this throughout `libpebble`, but we can't use the actual crate from `std` as
//! it complains that it can't find `core` (because we'd need to do the whole `rustc-std-workspace-core` charade
//! from `bit_field`. If that changes, we should remove this).
//!
//! This module is dual-licensed under the MIT and Apache-2.0 licenses. `Copyright (c) 2016 Philipp Oppermann`. The
//! original code can be found [here](https://github.com/phil-opp/rust-bit-field).

use core::ops::{Bound, Range, RangeBounds};

/// A generic trait which provides methods for extracting and setting specific bits or ranges of
/// bits.
pub trait BitField {
    /// The number of bits in this bit field.
    ///
    /// ```rust
    /// use bit_field::BitField;
    ///
    /// assert_eq!(u32::BIT_LENGTH, 32);
    /// assert_eq!(u64::BIT_LENGTH, 64);
    /// ```
    const BIT_LENGTH: usize;

    /// Obtains the bit at the index `bit`; note that index 0 is the least significant bit, while
    /// index `length() - 1` is the most significant bit.
    ///
    /// ```rust
    /// use bit_field::BitField;
    ///
    /// let value: u32 = 0b110101;
    ///
    /// assert_eq!(value.get_bit(1), false);
    /// assert_eq!(value.get_bit(2), true);
    /// ```
    ///
    /// ## Panics
    ///
    /// This method will panic if the bit index is out of bounds of the bit field.
    fn get_bit(&self, bit: usize) -> bool;

    /// Obtains the range of bits specified by `range`; note that index 0 is the least significant
    /// bit, while index `length() - 1` is the most significant bit.
    ///
    /// ```rust
    /// use bit_field::BitField;
    ///
    /// let value: u32 = 0b110101;
    ///
    /// assert_eq!(value.get_bits(0..3), 0b101);
    /// assert_eq!(value.get_bits(2..6), 0b1101);
    /// assert_eq!(value.get_bits(..), 0b110101);
    /// assert_eq!(value.get_bits(3..=3), value.get_bit(3) as u32);
    /// ```
    ///
    /// ## Panics
    ///
    /// This method will panic if the start or end indexes of the range are out of bounds of the
    /// bit field.
    fn get_bits<T: RangeBounds<usize>>(&self, range: T) -> Self;

    /// Sets the bit at the index `bit` to the value `value` (where true means a value of '1' and
    /// false means a value of '0'); note that index 0 is the least significant bit, while index
    /// `length() - 1` is the most significant bit.
    ///
    /// ```rust
    /// use bit_field::BitField;
    ///
    /// let mut value = 0u32;
    ///
    /// value.set_bit(1, true);
    /// assert_eq!(value, 2u32);
    ///
    /// value.set_bit(3, true);
    /// assert_eq!(value, 10u32);
    ///
    /// value.set_bit(1, false);
    /// assert_eq!(value, 8u32);
    /// ```
    ///
    /// ## Panics
    ///
    /// This method will panic if the bit index is out of the bounds of the bit field.
    fn set_bit(&mut self, bit: usize, value: bool) -> &mut Self;

    /// Sets the range of bits defined by the range `range` to the lower bits of `value`; to be
    /// specific, if the range is N bits long, the N lower bits of `value` will be used; if any of
    /// the other bits in `value` are set to 1, this function will panic.
    ///
    /// ```rust
    /// use bit_field::BitField;
    ///
    /// let mut value = 0u32;
    ///
    /// value.set_bits(0..2, 0b11);
    /// assert_eq!(value, 0b11);
    ///
    /// value.set_bits(2..=3, 0b11);
    /// assert_eq!(value, 0b1111);
    ///
    /// value.set_bits(..4, 0b1010);
    /// assert_eq!(value, 0b1010);
    /// ```
    ///
    /// ## Panics
    ///
    /// This method will panic if the range is out of bounds of the bit field, or if there are `1`s
    /// not in the lower N bits of `value`.
    fn set_bits<T: RangeBounds<usize>>(&mut self, range: T, value: Self) -> &mut Self;
}

/// An internal macro used for implementing BitField on the standard integral types.
macro_rules! bitfield_numeric_impl {
    ($($t:ty)*) => ($(
        impl BitField for $t {
            const BIT_LENGTH: usize = ::core::mem::size_of::<Self>() as usize * 8;

            #[inline]
            fn get_bit(&self, bit: usize) -> bool {
                assert!(bit < Self::BIT_LENGTH);

                (*self & (1 << bit)) != 0
            }

            #[inline]
            fn get_bits<T: RangeBounds<usize>>(&self, range: T) -> Self {
                let range = to_regular_range(&range, Self::BIT_LENGTH);

                assert!(range.start < Self::BIT_LENGTH);
                assert!(range.end <= Self::BIT_LENGTH);
                assert!(range.start < range.end);

                // shift away high bits
                let bits = *self << (Self::BIT_LENGTH - range.end) >> (Self::BIT_LENGTH - range.end);

                // shift away low bits
                bits >> range.start
            }

            #[inline]
            fn set_bit(&mut self, bit: usize, value: bool) -> &mut Self {
                assert!(bit < Self::BIT_LENGTH);

                if value {
                    *self |= 1 << bit;
                } else {
                    *self &= !(1 << bit);
                }

                self
            }

            #[inline]
            fn set_bits<T: RangeBounds<usize>>(&mut self, range: T, value: Self) -> &mut Self {
                let range = to_regular_range(&range, Self::BIT_LENGTH);

                assert!(range.start < Self::BIT_LENGTH);
                assert!(range.end <= Self::BIT_LENGTH);
                assert!(range.start < range.end);
                assert!(value << (Self::BIT_LENGTH - (range.end - range.start)) >>
                        (Self::BIT_LENGTH - (range.end - range.start)) == value,
                        "value does not fit into bit range");

                let bitmask: Self = !(!0 << (Self::BIT_LENGTH - range.end) >>
                                    (Self::BIT_LENGTH - range.end) >>
                                    range.start << range.start);

                // set bits
                *self = (*self & bitmask) | (value << range.start);

                self
            }
        }
    )*)
}

bitfield_numeric_impl! { u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize }

fn to_regular_range<T: RangeBounds<usize>>(generic_rage: &T, bit_length: usize) -> Range<usize> {
    let start = match generic_rage.start_bound() {
        Bound::Excluded(&value) => value + 1,
        Bound::Included(&value) => value,
        Bound::Unbounded => 0,
    };
    let end = match generic_rage.end_bound() {
        Bound::Excluded(&value) => value,
        Bound::Included(&value) => value + 1,
        Bound::Unbounded => bit_length,
    };

    start..end
}
