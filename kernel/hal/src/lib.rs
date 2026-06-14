#![no_std]
#![feature(pattern, trim_prefix_suffix, const_trait_impl)]

pub mod bootinfo;
pub mod cmdline;
pub mod cpu;
pub mod io;
pub mod mem;
pub mod sync;

/// Align `x` to `align`, where `align` is a power-of-2 or `0`, where the result will be `<=x`.
pub fn align_down(x: usize, align: usize) -> usize {
    if align.is_power_of_two() {
        /*
         * E.g.
         *      align       =   0b00001000
         *      align-1     =   0b00000111
         *      !(align-1)  =   0b11111000
         *                             ^^^ Masks the address to the value below it with the
         *                                 correct alignment
         */
        x & !(align - 1)
    } else {
        assert!(align == 0);
        x
    }
}

/// Align `x` to `align`, where `align` is a power-of-2 or `0`, where the result will be `>=x`.
pub fn align_up(x: usize, align: usize) -> usize {
    align_down(x + align - 1, align)
}
