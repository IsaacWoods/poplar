use cfg_if::cfg_if;
use core::{
    fmt,
    ops::{Add, AddAssign, Sub, SubAssign},
};

/// Represents a virtual address. On architectures that have extra requirements for canonical virtual addresses
/// (e.g. x86_64 requiring correct sign-extension in high bits), these requirements are always enforced.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct VirtualAddress(usize);

impl VirtualAddress {
    /// Construct a new `VirtualAddress`. This will canonicalise the given value.
    pub const fn new(address: usize) -> VirtualAddress {
        VirtualAddress(address).canonicalise()
    }

    pub const fn ptr<T>(self) -> *const T {
        self.0 as *const T
    }

    pub const fn mut_ptr<T>(self) -> *mut T {
        self.0 as *mut T
    }

    /*
     * How we canonicalise addresses is architecture-specific, but has leaked into `hal` to make the types
     * simpler to use. We enforce whatever requirements are needed for the target architecture.
     */
    cfg_if! {
        if #[cfg(target_arch = "x86_64")] {
            /// Canonicalise this virtual address. On x86_64, that involves making sure that bits 48..63 match the
            /// sign extension expected from the value of bit 47.
            pub const fn canonicalise(self) -> VirtualAddress {
                #[allow(inconsistent_digit_grouping)]
                const SIGN_EXTENSION: usize = 0o177777_000_000_000_000_0000;

                VirtualAddress((SIGN_EXTENSION * ((self.0 >> 47) & 0b1)) | (self.0 & ((1 << 48) - 1)))
            }
        } else {
            /// Canonicalise this virtual address. On this architecture, there are no extra requirements, and so we
            /// just return the address as is.
            pub const fn canonicalise(self) -> VirtualAddress {
                self
            }
        }
    }

    /// Align this address to the given alignment, moving downwards if this is not already aligned. `align` must
    /// be `0` or a power-of-two.
    pub fn align_down(self, align: usize) -> VirtualAddress {
        if align.is_power_of_two() {
            /*
             * E.g.
             *      align       =   0b00001000
             *      align-1     =   0b00000111
             *      !(align-1)  =   0b11111000
             *                             ^^^ Masks the address to the value below it with the
             *                                 correct alignment
             */
            VirtualAddress(self.0 & !(align - 1))
        } else {
            assert!(align == 0);
            self
        }
    }

    /// Align this address to the given alignment, moving upwards if this is not already aligned. `align` must be
    /// `0` or a power-of-two.
    pub fn align_up(self, align: usize) -> VirtualAddress {
        VirtualAddress(self.0 + align - 1).align_down(align)
    }

    pub fn is_aligned(self, align: usize) -> bool {
        self.0 % align == 0
    }

    pub fn checked_add(self, rhs: usize) -> Option<Self> {
        Some(VirtualAddress::new(self.0.checked_add(rhs)?))
    }

    pub fn checked_sub(self, rhs: usize) -> Option<Self> {
        Some(VirtualAddress::new(self.0.checked_sub(rhs)?))
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
        write!(f, "VirtualAddress({:#x})", self)
    }
}

impl From<VirtualAddress> for usize {
    fn from(address: VirtualAddress) -> usize {
        address.0
    }
}

impl<T> From<*const T> for VirtualAddress {
    fn from(ptr: *const T) -> VirtualAddress {
        VirtualAddress::new(ptr as usize)
    }
}

impl<T> From<*mut T> for VirtualAddress {
    fn from(ptr: *mut T) -> VirtualAddress {
        VirtualAddress::new(ptr as usize)
    }
}

impl Add<usize> for VirtualAddress {
    type Output = VirtualAddress;

    fn add(self, rhs: usize) -> Self::Output {
        VirtualAddress::new(self.0 + rhs)
    }
}

impl AddAssign<usize> for VirtualAddress {
    fn add_assign(&mut self, rhs: usize) {
        // XXX: this ensures correctness as it goes through the `Add` implementation
        *self = *self + rhs;
    }
}

impl Sub<usize> for VirtualAddress {
    type Output = VirtualAddress;

    fn sub(self, rhs: usize) -> Self::Output {
        VirtualAddress::new(self.0 - rhs)
    }
}

impl SubAssign<usize> for VirtualAddress {
    fn sub_assign(&mut self, rhs: usize) {
        // XXX: this ensures correctness as it goes through the `Sub` implementation
        *self = *self - rhs;
    }
}
