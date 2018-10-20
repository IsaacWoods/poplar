use super::paging::PAGE_SIZE;
use core::cmp::Ordering;
use core::fmt;
use core::ops::{Add, Sub};

/// Represents a canonical virtual address. To be canonical, the address must be in the ranges
/// `0x0000_0000_0000_0000` to `0x0000_8000_0000_0000` or `0xffff_8000_0000_0000` to
/// `0xffff_ffff_ffff_ffff`.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct VirtualAddress(u64);

impl VirtualAddress {
    /// Create a new `VirtualAddress` from the given address. If the given address is not a valid
    /// canonical address, this returns `None`.
    /*
     * TODO: this should be made `const` when CTFE supports matches, then we should use it for all
     * the constants that currently use `new_unchecked`
     */
    pub fn new(address: u64) -> Option<VirtualAddress> {
        match address {
            0x0000_0000_0000_0000..=0x0000_7fff_ffff_ffff => Some(VirtualAddress(address)),
            0xffff_8000_0000_0000..=0xffff_ffff_ffff_ffff => Some(VirtualAddress(address)),
            _ => None,
        }
    }

    /// Create a new `VirtualAddress` from the given address, which is assumed to be canonical.
    /// Unsafe because using a non-canonical address can cause General Protection faults.
    pub const unsafe fn new_unchecked(address: u64) -> VirtualAddress {
        VirtualAddress(address)
    }

    pub const fn from_page_table_offsets(
        p4: u16,
        p3: u16,
        p2: u16,
        p1: u16,
        offset: usize,
    ) -> VirtualAddress {
        VirtualAddress(
            ((p4 as u64) << 39)
                | ((p3 as u64) << 30)
                | ((p2 as u64) << 21)
                | ((p1 as u64) << 12)
                | ((offset as u64) << 0),
        )
        .canonicalise()
    }

    pub const fn ptr<T>(self) -> *const T {
        self.0 as *const T
    }

    pub const fn mut_ptr<T>(self) -> *mut T {
        self.0 as *mut T
    }

    pub const fn offset(&self, offset: i64) -> VirtualAddress {
        VirtualAddress(((self.0 as i64) + offset) as u64).canonicalise()
    }

    pub const fn is_page_aligned(&self) -> bool {
        self.0 % PAGE_SIZE == 0
    }

    pub const fn is_aligned_to(&self, alignment: u64) -> bool {
        self.0 % alignment == 0
    }

    pub const fn offset_into_page(&self) -> u64 {
        self.0 % PAGE_SIZE
    }

    /// Addresses are always expected by the CPU to be canonical (bits 48 to 63 are the same as bit
    /// 47). If a calculation leaves an address non-canonical, make sure to re-canonicalise it with
    /// this function.
    pub const fn canonicalise(self) -> VirtualAddress {
        #[allow(inconsistent_digit_grouping)]
        const SIGN_EXTENSION: u64 = 0o177777_000_000_000_000_0000;

        VirtualAddress((SIGN_EXTENSION * ((self.0 >> 47) & 0b1)) | (self.0 & ((1 << 48) - 1)))
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

impl From<u64> for VirtualAddress {
    fn from(address: u64) -> VirtualAddress {
        VirtualAddress(address)
    }
}

impl From<VirtualAddress> for u64 {
    fn from(address: VirtualAddress) -> u64 {
        address.0
    }
}

impl<T> From<*const T> for VirtualAddress {
    fn from(ptr: *const T) -> VirtualAddress {
        (ptr as u64).into()
    }
}

impl<T> From<*mut T> for VirtualAddress {
    fn from(ptr: *mut T) -> VirtualAddress {
        (ptr as u64).into()
    }
}

impl Add<VirtualAddress> for VirtualAddress {
    type Output = VirtualAddress;

    fn add(self, rhs: VirtualAddress) -> VirtualAddress {
        (self.0 + rhs.0).into()
    }
}

impl Sub<VirtualAddress> for VirtualAddress {
    type Output = VirtualAddress;

    fn sub(self, rhs: VirtualAddress) -> VirtualAddress {
        (self.0 - rhs.0).into()
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
