/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

use core::ops;
use num_traits::PrimInt;

// TODO: feels like something like this should exist in `num_traits` or something, but if it does I couldn't find
// it
pub trait PowerOfTwoable {
    fn is_power_of_two(self) -> bool;
    fn next_power_of_two(self) -> Self;
}

macro impl_power_of_twoable($type:ty) {
    impl PowerOfTwoable for $type {
        fn is_power_of_two(self) -> bool {
            self.is_power_of_two()
        }

        fn next_power_of_two(self) -> Self {
            self.next_power_of_two()
        }
    }
}

impl_power_of_twoable!(u8);
impl_power_of_twoable!(u16);
impl_power_of_twoable!(u32);
impl_power_of_twoable!(u64);
impl_power_of_twoable!(usize);

pub fn align_down<T: PrimInt + PowerOfTwoable>(value: T, align: T) -> T {
    assert!(align == T::zero() || align.is_power_of_two());

    if align == T::zero() {
        value
    } else {
        /*
         * Alignment must be a power of two.
         *
         * E.g.
         * align       =   0b00001000
         * align-1     =   0b00000111
         * !(align-1)  =   0b11111000
         * ^^^ Masks the value to the one below it with the correct align
         */
        value & !(align - T::one())
    }
}

#[test]
fn test_align_down() {
    assert_eq!(align_down(17u64, 0), 17);
    assert_eq!(align_down(17u64, 1), 17);
    assert_eq!(align_down(9u64, 8), 8);
    assert_eq!(align_down(19u64, 8), 16);
    assert_eq!(align_down(1025u64, 16), 1024);
}

pub fn align_up<T: PrimInt + PowerOfTwoable>(value: T, align: T) -> T {
    if align == T::zero() {
        value
    } else {
        align_down(value + align - T::one(), align)
    }
}

#[test]
fn test_align_up() {
    assert_eq!(align_up(17u64, 0), 17);
    assert_eq!(align_up(43u64, 1), 43);
    assert_eq!(align_up(9u64, 8), 16);
    assert_eq!(align_up(1023u64, 16), 1024);
}

/// Divide `x` by `divide_by`, taking the ceiling if it does not divide evenly.
pub fn ceiling_integer_divide(x: usize, divide_by: usize) -> usize {
    x / divide_by + if x % divide_by != 0 { 1 } else { 0 }
}

#[test]
fn test_ceiling_integer_divide() {
    assert_eq!(ceiling_integer_divide(1, 1), 1);
    assert_eq!(ceiling_integer_divide(10, 5), 2);
    assert_eq!(ceiling_integer_divide(11, 5), 3);
    assert_eq!(ceiling_integer_divide(0, 5), 0);
}

pub fn abs_difference<T>(a: T, b: T) -> T
where
    T: Ord + ops::Sub<Output = T>,
{
    if a > b {
        a - b
    } else {
        b - a
    }
}
