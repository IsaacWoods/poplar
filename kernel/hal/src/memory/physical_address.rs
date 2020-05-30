use cfg_if::cfg_if;
use core::{
    fmt,
    ops::{Add, AddAssign, Sub, SubAssign},
};

/// Represents a physical address. If the target architecture has any requirements for valid physical addresses,
/// they must always be observed by values of this type.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct PhysicalAddress(usize);

impl PhysicalAddress {
    cfg_if! {
        if #[cfg(target_arch = "x86_64")] {
            /// On x86_64, physical addresses must be less than 2^52.
            pub const fn new(address: usize) -> Option<PhysicalAddress> {
                const MAX_PHYSICAL_ADDRESS: usize = (1 << 52) - 1;
                match address {
                    0..=MAX_PHYSICAL_ADDRESS => Some(PhysicalAddress(address)),
                    _ => None
                }
            }
        } else {
            /// Construct a new `PhysicalAddress`. The target architecture does not have any requirements on valid
            /// physical addresses, so this always succeeds.
            pub const fn new(address: usize) -> Option<PhysicalAddress> {
                Some(PhysicalAddress(address))
            }
        }
    }

    /// Align this address to the given alignment, moving downwards if this is not already aligned.
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

    pub fn is_aligned(self, align: usize) -> bool {
        self.0 % align == 0
    }

    pub fn checked_add(self, rhs: usize) -> Option<Self> {
        PhysicalAddress::new(self.0.checked_add(rhs)?)
    }

    pub fn checked_sub(self, rhs: usize) -> Option<Self> {
        PhysicalAddress::new(self.0.checked_sub(rhs)?)
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
