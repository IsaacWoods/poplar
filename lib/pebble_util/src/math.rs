use cfg_if::cfg_if;

cfg_if! {
    if #[cfg(feature = "tuning-fast_ctlz")] {
        /// Fast integer `log2` that floors to the lower power-of-2 if `x` is not a power-of-2. `x`
        /// must not be 0.
        ///
        /// ### Example
        /// ```
        /// assert_eq!(flooring_log2(1), 0);
        /// assert_eq!(flooring_log2(64), 6);
        /// assert_eq!(flooring_log2(61), 5);
        /// assert_eq!(flooring_log2(4095), 11);
        /// ```
        pub fn flooring_log2(mut x: u64) -> u64 {
            assert!(x > 0);

            /*
             * Count the number of leading zeros in the value, then subtract that from the total
             * number of bits in the type (64 for a `u64`). This gets the first bit set, which is
             * the largest power-of-2 component of the value.
             */
            return 64 - (unsafe { core::intrinsics::ctlz(x) } + 1);
        }
    } else {
        /// Fast integer `log2` that floors to the lower power-of-2 if `x` is not a power-of-2. `x`
        /// must not be 0.
        ///
        /// ### Example
        /// ```
        /// assert_eq!(flooring_log2(1), 0);
        /// assert_eq!(flooring_log2(64), 6);
        /// assert_eq!(flooring_log2(61), 5);
        /// assert_eq!(flooring_log2(4095), 11);
        /// ```
        pub fn flooring_log2(mut x: u64) -> u64 {
            assert!(x > 0);

            /*
             * Use a de-Bruijn-like algorithm, which should be the fastest CPU-agnostic way to find
             * the previous power-of-2.
             */
            const TABLE: [u8; 64] = [
                63,  0, 58,  1, 59, 47, 53,  2,
                60, 39, 48, 27, 54, 33, 42,  3,
                61, 51, 37, 40, 49, 18, 28, 20,
                55, 30, 34, 11, 43, 14, 22,  4,
                62, 57, 46, 52, 38, 26, 32, 41,
                50, 36, 17, 19, 29, 10, 13, 21,
                56, 45, 25, 31, 35, 16,  9, 12,
                44, 24, 15,  8, 23,  7,  6,  5,
            ];

            x |= x >> 1;
            x |= x >> 2;
            x |= x >> 4;
            x |= x >> 8;
            x |= x >> 16;
            x |= x >> 32;

            /*
             * Casting to `usize` here is fine even on 32-bit platforms because the slice is only
             * 64 elements long anyways.
             */
            TABLE[((((x - (x >> 1)).wrapping_mul(0x07edd5e59a4e28c2))) >> 58) as usize] as u64
        }
    }
}

#[test]
fn test_flooring_log2() {
    assert_eq!(flooring_log2(1), 0);
    assert_eq!(flooring_log2(64), 6);
    assert_eq!(flooring_log2(61), 5);
    assert_eq!(flooring_log2(4095), 11);
}

pub fn align_down(value: usize, align: usize) -> usize {
    assert!(align == 0 || align.is_power_of_two());

    if align == 0 {
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
        value & !(align - 1)
    }
}

#[test]
fn test_align_down() {
    assert_eq!(align_down(17, 0), 17);
    assert_eq!(align_down(17, 1), 17);
    assert_eq!(align_down(9, 8), 8);
    assert_eq!(align_down(19, 8), 16);
    assert_eq!(align_down(1025, 16), 1024);
}

pub fn align_up(value: usize, align: usize) -> usize {
    if align == 0 {
        value
    } else {
        align_down(value + align - 1, align)
    }
}

#[test]
fn test_align_up() {
    assert_eq!(align_up(17, 0), 17);
    assert_eq!(align_up(43, 1), 43);
    assert_eq!(align_up(9, 8), 16);
    assert_eq!(align_up(1023, 16), 1024);
}

pub fn ceiling_log2(x: u64) -> u64 {
    let x = if x.is_power_of_two() { x } else { x.next_power_of_two() };

    // `x` will always be a power of two now, so log(2) == the number of trailing zeros
    x.trailing_zeros() as u64
}

#[test]
fn test_ceiling_log2() {
    assert_eq!(ceiling_log2(1), 0);
    assert_eq!(ceiling_log2(64), 6);
    assert_eq!(ceiling_log2(61), 6);
    assert_eq!(ceiling_log2(4095), 12);
}

/// Divide `x` by `divide_by`, taking the ceiling if it does not divide evenly.
pub fn ceiling_integer_divide(x: u64, divide_by: u64) -> u64 {
    x / divide_by + if x % divide_by != 0 { 1 } else { 0 }
}

#[test]
fn test_ceiling_integer_divide() {
    assert_eq!(ceiling_integer_divide(1, 1), 1);
    assert_eq!(ceiling_integer_divide(10, 5), 2);
    assert_eq!(ceiling_integer_divide(11, 5), 3);
    assert_eq!(ceiling_integer_divide(0, 5), 0);
}
