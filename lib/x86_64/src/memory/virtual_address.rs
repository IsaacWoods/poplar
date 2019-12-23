use super::FrameSize;
use bit_field::BitField;
use core::{
    cmp::Ordering,
    fmt,
    ops::{Add, AddAssign, Sub, SubAssign},
};

/// Represents a canonical virtual address. To be canonical, the address must be in the ranges
/// `0x0000_0000_0000_0000` to `0x0000_8000_0000_0000` or `0xffff_8000_0000_0000` to
/// `0xffff_ffff_ffff_ffff`.
#[derive(Clone, Copy, Default)]
#[repr(transparent)]
pub struct VirtualAddress(usize);

impl VirtualAddress {
    /// Create a new `VirtualAddress` from the given address. If the given address is not a valid
    /// canonical address, this returns `None`.
    /*
     * TODO: this should be made `const` when CTFE supports matches, then we should use it for
     * all the constants that currently use `new_unchecked`
     */
    pub fn new(address: usize) -> Option<VirtualAddress> {
        match address {
            0x0000_0000_0000_0000..=0x0000_7fff_ffff_ffff => Some(VirtualAddress(address)),
            0xffff_8000_0000_0000..=0xffff_ffff_ffff_ffff => Some(VirtualAddress(address)),
            _ => None,
        }
    }

    /// Create a new `VirtualAddress` from the given address, which is assumed to be canonical.
    /// Unsafe because using a non-canonical address can cause General Protection faults.
    pub const unsafe fn new_unchecked(address: usize) -> VirtualAddress {
        VirtualAddress(address)
    }

    /// Create a new `VirtualAddress` from the given address, canonicalising it if it is not
    /// already canonical, by the logic in the `VirtualAddress::canonicalise` method.
    pub const fn new_canonicalise(address: usize) -> VirtualAddress {
        VirtualAddress(address).canonicalise()
    }

    pub const fn from_page_table_offsets(
        p4: usize,
        p3: usize,
        p2: usize,
        p1: usize,
        offset: usize,
    ) -> VirtualAddress {
        VirtualAddress((p4 << 39) | (p3 << 30) | (p2 << 21) | (p1 << 12) | (offset << 0)).canonicalise()
    }

    pub const fn ptr<T>(self) -> *const T {
        self.0 as *const T
    }

    pub const fn mut_ptr<T>(self) -> *mut T {
        self.0 as *mut T
    }

    pub const fn offset(&self, offset: isize) -> VirtualAddress {
        VirtualAddress(((self.0 as isize) + offset) as usize).canonicalise()
    }

    pub const fn offset_into_page<S: FrameSize>(&self) -> usize {
        self.0 % S::SIZE
    }

    pub const fn is_page_aligned<S: FrameSize>(&self) -> bool {
        self.offset_into_page::<S>() == 0
    }

    pub const fn is_aligned_to(&self, alignment: usize) -> bool {
        self.0 % alignment == 0
    }

    /// Get the greatest address `x` with the given alignment such that `x <= self`. The alignment
    /// must be `0` or a power of two.
    pub fn align_down(&self, align: usize) -> VirtualAddress {
        assert!(align == 0 || align.is_power_of_two());

        if align == 0 {
            *self
        } else {
            /*
             * The alignment is a power of two, so we just:
             *   e.g. align         = 0b00001000
             *        align - 1     = 0b00000111
             *        !(align - 1)  = 0b11111000
             *                               ^^^ Masks the address to the address below it with
             *                                   the correct alignment
             */
            VirtualAddress::new_canonicalise(self.0 & !(align - 1))
        }
    }

    /// Get the smallest address `x` with the given alignment such that `x >= self`. The alignment
    /// must be `0` or a power of two.
    pub fn align_up(&self, align: usize) -> VirtualAddress {
        (*self + (align - 1)).align_down(align)
    }

    /// Addresses are always expected by the CPU to be canonical (bits 48 to 63 are the same as bit
    /// 47). If a calculation leaves an address non-canonical, make sure to re-canonicalise it with
    /// this function.
    pub const fn canonicalise(self) -> VirtualAddress {
        #[allow(inconsistent_digit_grouping)]
        const SIGN_EXTENSION: usize = 0o177777_000_000_000_000_0000;

        VirtualAddress((SIGN_EXTENSION * ((self.0 >> 47) & 0b1)) | (self.0 & ((1 << 48) - 1)))
    }

    pub fn p4_index(&self) -> usize {
        self.0.get_bits(39..48)
    }

    pub fn p3_index(&self) -> usize {
        self.0.get_bits(30..39)
    }

    pub fn p2_index(&self) -> usize {
        self.0.get_bits(21..30)
    }

    pub fn p1_index(&self) -> usize {
        self.0.get_bits(12..21)
    }
}

impl fmt::LowerHex for VirtualAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

impl fmt::UpperHex for VirtualAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#X}", self.0)
    }
}

impl fmt::Debug for VirtualAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#x}", self)
    }
}

impl From<VirtualAddress> for usize {
    fn from(address: VirtualAddress) -> usize {
        address.0
    }
}

impl<T> From<*const T> for VirtualAddress {
    fn from(ptr: *const T) -> VirtualAddress {
        VirtualAddress::new(ptr as usize).expect("Pointer is non-canonical!")
    }
}

impl<T> From<*mut T> for VirtualAddress {
    fn from(ptr: *mut T) -> VirtualAddress {
        VirtualAddress::new(ptr as usize).expect("Pointer is non-canonical!")
    }
}

impl AddAssign<usize> for VirtualAddress {
    fn add_assign(&mut self, rhs: usize) {
        *self = *self + rhs;
    }
}

impl Add<usize> for VirtualAddress {
    type Output = VirtualAddress;

    fn add(self, rhs: usize) -> Self::Output {
        VirtualAddress::new_canonicalise(self.0 + rhs)
    }
}

impl Sub<usize> for VirtualAddress {
    type Output = VirtualAddress;

    fn sub(self, rhs: usize) -> Self::Output {
        VirtualAddress::new_canonicalise(self.0 - rhs)
    }
}

impl SubAssign<usize> for VirtualAddress {
    fn sub_assign(&mut self, rhs: usize) {
        *self = *self - rhs;
    }
}

impl PartialEq<VirtualAddress> for VirtualAddress {
    fn eq(&self, other: &VirtualAddress) -> bool {
        self.0 == other.0
    }
}

impl Eq for VirtualAddress {}

impl PartialOrd<VirtualAddress> for VirtualAddress {
    fn partial_cmp(&self, rhs: &VirtualAddress) -> Option<Ordering> {
        self.0.partial_cmp(&rhs.0)
    }
}

impl Ord for VirtualAddress {
    fn cmp(&self, rhs: &VirtualAddress) -> Ordering {
        self.0.cmp(&rhs.0)
    }
}

#[cfg(test)]
mod tests {
    use crate::memory::*;

    #[test]
    fn test_page_table_offsets() {
        let start_of_kernel_space = VirtualAddress::from_page_table_offsets(511, 0, 0, 0, 0);
        assert_eq!(start_of_kernel_space.p4_index(), 511);
        assert_eq!(start_of_kernel_space.p3_index(), 0);
        assert_eq!(start_of_kernel_space.p2_index(), 0);
        assert_eq!(start_of_kernel_space.p1_index(), 0);
        assert_eq!(start_of_kernel_space.offset_into_page::<Size4KiB>(), 0);
    }
}
