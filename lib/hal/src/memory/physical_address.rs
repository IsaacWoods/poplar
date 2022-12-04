use cfg_if::cfg_if;
use core::{
    fmt,
    ops::{Add, AddAssign, Sub, SubAssign},
};

/// Represents a physical address. If the target architecture has any requirements for valid physical addresses,
/// they are always enforced.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct PAddr(usize);

impl PAddr {
    cfg_if! {
        if #[cfg(target_arch = "x86_64")] {
            /// Construct a new `PAddr`. On x86_64, physical addresses must be less than `2^52`; if this is
            /// not the case, this will return `None`.
            pub const fn new(address: usize) -> Option<PAddr> {
                const MAX_PHYSICAL_ADDRESS: usize = (1 << 52) - 1;
                match address {
                    0..=MAX_PHYSICAL_ADDRESS => Some(PAddr(address)),
                    _ => None
                }
            }
        } else {
            /// Construct a new `PAddr`. The target architecture does not have any requirements on valid
            /// physical addresses, so this always succeeds.
            pub const fn new(address: usize) -> Option<PAddr> {
                Some(PAddr(address))
            }
        }
    }

    /// Align this address to the given alignment, moving downwards if this is not already aligned.
    /// `align` must be `0` or a power-of-two.
    pub fn align_down(self, align: usize) -> PAddr {
        if align.is_power_of_two() {
            /*
             * E.g.
             *      align       =   0b00001000
             *      align-1     =   0b00000111
             *      !(align-1)  =   0b11111000
             *                             ^^^ Masks the address to the value below it with the
             *                                 correct alignment
             */
            PAddr(self.0 & !(align - 1))
        } else {
            assert!(align == 0);
            self
        }
    }

    pub fn align_up(self, align: usize) -> PAddr {
        PAddr(self.0 + align - 1).align_down(align)
    }

    pub fn is_aligned(self, align: usize) -> bool {
        self.0 % align == 0
    }

    pub fn checked_add(self, rhs: usize) -> Option<Self> {
        PAddr::new(self.0.checked_add(rhs)?)
    }

    pub fn checked_sub(self, rhs: usize) -> Option<Self> {
        PAddr::new(self.0.checked_sub(rhs)?)
    }
}

impl fmt::LowerHex for PAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

impl fmt::UpperHex for PAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#X}", self.0)
    }
}

impl fmt::Debug for PAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PAddr({:#x})", self)
    }
}

impl From<PAddr> for usize {
    fn from(address: PAddr) -> usize {
        address.0
    }
}

impl Add<usize> for PAddr {
    type Output = PAddr;

    fn add(self, rhs: usize) -> Self::Output {
        match PAddr::new(self.0 + rhs) {
            Some(address) => address,
            None => panic!("Physical address arithmetic led to invalid address: {:#x} + {:#x}", self, rhs),
        }
    }
}

impl AddAssign<usize> for PAddr {
    fn add_assign(&mut self, rhs: usize) {
        // XXX: this ensures correctness as it goes through the `Add` implementation
        *self = *self + rhs;
    }
}

impl Sub<usize> for PAddr {
    type Output = PAddr;

    fn sub(self, rhs: usize) -> Self::Output {
        match PAddr::new(self.0 - rhs) {
            Some(address) => address,
            None => panic!("Physical address arithmetic led to invalid address: {:#x} - {:#x}", self, rhs),
        }
    }
}

impl SubAssign<usize> for PAddr {
    fn sub_assign(&mut self, rhs: usize) {
        // XXX: this ensures correctness as it goes through the `Sub` implementation
        *self = *self - rhs;
    }
}
