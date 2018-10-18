use super::paging::FRAME_SIZE;
use core::cmp::Ordering;
use core::fmt;
use core::ops::{Add, Sub};

#[derive(Clone, Copy)]
pub struct PhysicalAddress(usize);

impl PhysicalAddress {
    pub const fn new(address: usize) -> PhysicalAddress {
        PhysicalAddress(address)
    }

    pub const fn offset(&self, offset: isize) -> PhysicalAddress {
        PhysicalAddress::new(((self.0 as isize) + offset) as usize)
    }

    pub const fn offset_into_frame(&self) -> usize {
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

impl From<usize> for PhysicalAddress {
    fn from(address: usize) -> PhysicalAddress {
        PhysicalAddress(address)
    }
}

impl From<PhysicalAddress> for usize {
    fn from(address: PhysicalAddress) -> usize {
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
