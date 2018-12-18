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
