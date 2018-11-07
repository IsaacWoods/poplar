use super::paging::FRAME_SIZE;
use core::cmp::Ordering;
use core::fmt;
use core::ops::{Add, Sub};

/// Represents an address in the physical memory space. A valid physical address is smaller than
/// 2^52
#[derive(Clone, Copy, Default)]
#[repr(transparent)]
pub struct PhysicalAddress(u64);

impl PhysicalAddress {
    // TODO: make `const` when const match is supported
    pub fn new(address: u64) -> Option<PhysicalAddress> {
        // This constant has to exist because we can't use expressions in a range
        const MAX_PHYSICAL_ADDRESS: u64 = (1 << 52) - 1;

        match address {
            0..=MAX_PHYSICAL_ADDRESS => Some(PhysicalAddress(address)),
            _ => None,
        }
    }

    pub const fn new_unchecked(address: u64) -> PhysicalAddress {
        PhysicalAddress(address)
    }

    pub const fn offset_into_frame(&self) -> u64 {
        self.0 % FRAME_SIZE
    }

    pub const fn is_frame_aligned(&self) -> bool {
        self.offset_into_frame() == 0
    }
}

impl fmt::LowerHex for PhysicalAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

impl fmt::UpperHex for PhysicalAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#X}", self.0)
    }
}

impl fmt::Debug for PhysicalAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PhysicalAddress({:#x})", self)
    }
}

impl From<PhysicalAddress> for u64 {
    fn from(address: PhysicalAddress) -> u64 {
        address.0
    }
}

impl Add<u64> for PhysicalAddress {
    type Output = Option<PhysicalAddress>;

    fn add(self, rhs: u64) -> Self::Output {
        PhysicalAddress::new(self.0 + rhs)
    }
}

impl Sub<u64> for PhysicalAddress {
    type Output = Option<PhysicalAddress>;

    fn sub(self, rhs: u64) -> Self::Output {
        PhysicalAddress::new(self.0 - rhs)
    }
}

impl PartialEq<PhysicalAddress> for PhysicalAddress {
    fn eq(&self, rhs: &PhysicalAddress) -> bool {
        self.0 == rhs.0
    }
}

impl Eq for PhysicalAddress {}

impl PartialOrd<PhysicalAddress> for PhysicalAddress {
    fn partial_cmp(&self, rhs: &PhysicalAddress) -> Option<Ordering> {
        self.0.partial_cmp(&rhs.0)
    }
}

impl Ord for PhysicalAddress {
    fn cmp(&self, rhs: &PhysicalAddress) -> Ordering {
        self.0.cmp(&rhs.0)
    }
}
