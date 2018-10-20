use super::paging::FRAME_SIZE;
use core::cmp::Ordering;
use core::fmt;
use core::ops::{Add, Sub};

/// Represents an address in the physical memory space.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct PhysicalAddress(u64);

impl PhysicalAddress {
    pub const fn new(address: u64) -> PhysicalAddress {
        PhysicalAddress(address)
    }

    pub const fn offset(&self, offset: i64) -> PhysicalAddress {
        PhysicalAddress::new(((self.0 as i64) + offset) as u64)
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

impl From<u64> for PhysicalAddress {
    fn from(address: u64) -> PhysicalAddress {
        PhysicalAddress(address)
    }
}

impl From<PhysicalAddress> for u64 {
    fn from(address: PhysicalAddress) -> u64 {
        address.0
    }
}

impl Add<PhysicalAddress> for PhysicalAddress {
    type Output = PhysicalAddress;

    fn add(self, rhs: PhysicalAddress) -> PhysicalAddress {
        (self.0 + rhs.0).into()
    }
}

impl Sub<PhysicalAddress> for PhysicalAddress {
    type Output = PhysicalAddress;

    fn sub(self, rhs: PhysicalAddress) -> PhysicalAddress {
        (self.0 - rhs.0).into()
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
