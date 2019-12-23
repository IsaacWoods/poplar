use super::FrameSize;
use core::{
    cmp::Ordering,
    fmt,
    ops::{Add, AddAssign, Sub, SubAssign},
};

/// Represents an address in the physical memory space. A valid physical address is smaller than
/// 2^52
#[derive(Clone, Copy, Default)]
#[repr(transparent)]
pub struct PhysicalAddress(usize);

impl PhysicalAddress {
    // TODO: make `const` when const match is supported
    pub fn new(address: usize) -> Option<PhysicalAddress> {
        // This constant has to exist because we can't use expressions in a range
        const MAX_PHYSICAL_ADDRESS: usize = (1 << 52) - 1;

        match address {
            0..=MAX_PHYSICAL_ADDRESS => Some(PhysicalAddress(address)),
            _ => None,
        }
    }

    pub const unsafe fn new_unchecked(address: usize) -> PhysicalAddress {
        PhysicalAddress(address)
    }

    pub const fn offset_into_frame<S: FrameSize>(&self) -> usize {
        self.0 % S::SIZE
    }

    pub const fn is_frame_aligned<S: FrameSize>(&self) -> bool {
        self.offset_into_frame::<S>() == 0
    }

    /// Get the greatest address less than or equal to this address with the given alignment.
    /// `align` must be `0` or a power-of-two.
    pub fn align_down(self, align: usize) -> PhysicalAddress {
        if align.is_power_of_two() {
            /*
             * E.g.
             *      align       =   0b00001000
             *      align-1     =   0b00000111
             *      !(align-1)  =   0b11111000
             *                             ^^^ Masks the address to the value below it with the
             *                                 correct alignment
             */
            PhysicalAddress(self.0 & !(align - 1))
        } else {
            assert!(align == 0);
            self
        }
    }

    pub fn align_up(self, align: usize) -> PhysicalAddress {
        PhysicalAddress(self.0 + align - 1).align_down(align)
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

impl From<PhysicalAddress> for usize {
    fn from(address: PhysicalAddress) -> usize {
        address.0
    }
}

impl Add<usize> for PhysicalAddress {
    type Output = PhysicalAddress;

    fn add(self, rhs: usize) -> Self::Output {
        match PhysicalAddress::new(self.0 + rhs) {
            Some(address) => address,
            None => panic!("Physical address arithmetic led to invalid address: {:#x} + {:#x}", self, rhs),
        }
    }
}

impl AddAssign<usize> for PhysicalAddress {
    fn add_assign(&mut self, rhs: usize) {
        *self = *self + rhs;
    }
}

impl Sub<usize> for PhysicalAddress {
    type Output = PhysicalAddress;

    fn sub(self, rhs: usize) -> Self::Output {
        match PhysicalAddress::new(self.0 - rhs) {
            Some(address) => address,
            None => panic!("Physical address arithmetic led to invalid address: {:#x} - {:#x}", self, rhs),
        }
    }
}

impl SubAssign<usize> for PhysicalAddress {
    fn sub_assign(&mut self, rhs: usize) {
        *self = *self - rhs;
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
