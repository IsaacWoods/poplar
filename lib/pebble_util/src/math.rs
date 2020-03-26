use core::mem;

/// Fast integer `log2` that floors to the lower power-of-2 if `x` is not a power-of-2. `x`
/// must not be 0.
///
/// ### Example
/// ``` ignore
/// assert_eq!(flooring_log2(1), 0);
/// assert_eq!(flooring_log2(64), 6);
/// assert_eq!(flooring_log2(61), 5);
/// assert_eq!(flooring_log2(4095), 11);
/// ```
pub fn flooring_log2(x: usize) -> usize {
    assert!(x > 0);

    /*
     * Count the number of leading zeros in the value, then subtract that from the total
     * number of bits in the type (64 for a `u64`). This gets the first bit set, which is
     * the largest power-of-2 component of the value.
     */
    return (8 * mem::size_of::<usize>()) - (unsafe { core::intrinsics::ctlz(x) } + 1);
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

pub fn ceiling_log2(x: usize) -> usize {
    let x = if x.is_power_of_two() { x } else { x.next_power_of_two() };

    // `x` will always be a power of two now, so log(2) == the number of trailing zeros
    x.trailing_zeros() as usize
}

#[test]
fn test_ceiling_log2() {
    assert_eq!(ceiling_log2(1), 0);
    assert_eq!(ceiling_log2(64), 6);
    assert_eq!(ceiling_log2(61), 6);
    assert_eq!(ceiling_log2(4095), 12);
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
