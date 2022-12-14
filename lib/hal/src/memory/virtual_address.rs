use cfg_if::cfg_if;
use core::{
    fmt,
    ops::{Add, AddAssign, Sub, SubAssign},
};

/// Represents a virtual address. On architectures that have extra requirements for canonical virtual addresses
/// (e.g. x86_64 requiring correct sign-extension in high bits), these requirements are always enforced.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct VAddr(usize);

impl VAddr {
    /// Construct a new `VAddr`. This will canonicalise the given value.
    pub const fn new(address: usize) -> VAddr {
        VAddr(address).canonicalise()
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
        if #[cfg(any(target_arch = "x86_64", target_arch = "riscv64"))] {
            /// Canonicalise this virtual address. On x86_64 and RV64-Sv48, that involves making sure that bits 48..63 match the
            /// sign extension expected from the value of bit 47.
            pub const fn canonicalise(self) -> VAddr {
                const SIGN_EXTENSION: usize = 0o177777_000_000_000_000_0000;

                VAddr((SIGN_EXTENSION * ((self.0 >> 47) & 0b1)) | (self.0 & ((1 << 48) - 1)))
            }
        } else {
            /// Canonicalise this virtual address. On this architecture, there are no extra requirements, and so we
            /// just return the address as is.
            pub const fn canonicalise(self) -> VAddr {
                self
            }
        }
    }

    /// Align this address to the given alignment, moving downwards if this is not already aligned. `align` must
    /// be `0` or a power-of-two.
    pub fn align_down(self, align: usize) -> VAddr {
        if align.is_power_of_two() {
            /*
             * E.g.
             *      align       =   0b00001000
             *      align-1     =   0b00000111
             *      !(align-1)  =   0b11111000
             *                             ^^^ Masks the address to the value below it with the
             *                                 correct alignment
             */
            VAddr(self.0 & !(align - 1))
        } else {
            assert!(align == 0);
            self
        }
    }

    /// Align this address to the given alignment, moving upwards if this is not already aligned. `align` must be
    /// `0` or a power-of-two.
    pub fn align_up(self, align: usize) -> VAddr {
        VAddr(self.0 + align - 1).align_down(align)
    }

    pub fn is_aligned(self, align: usize) -> bool {
        self.0 % align == 0
    }

    pub fn checked_add(self, rhs: usize) -> Option<Self> {
        Some(VAddr::new(self.0.checked_add(rhs)?))
    }

    pub fn checked_sub(self, rhs: usize) -> Option<Self> {
        Some(VAddr::new(self.0.checked_sub(rhs)?))
    }
}

impl fmt::LowerHex for VAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

impl fmt::UpperHex for VAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#X}", self.0)
    }
}

impl fmt::Debug for VAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "VAddr({:#x})", self)
    }
}

impl From<VAddr> for usize {
    fn from(address: VAddr) -> usize {
        address.0
    }
}

impl<T> From<*const T> for VAddr {
    fn from(ptr: *const T) -> VAddr {
        VAddr::new(ptr as usize)
    }
}

impl<T> From<*mut T> for VAddr {
    fn from(ptr: *mut T) -> VAddr {
        VAddr::new(ptr as usize)
    }
}

impl Add<usize> for VAddr {
    type Output = VAddr;

    fn add(self, rhs: usize) -> Self::Output {
        VAddr::new(self.0 + rhs)
    }
}

impl AddAssign<usize> for VAddr {
    fn add_assign(&mut self, rhs: usize) {
        // XXX: this ensures correctness as it goes through the `Add` implementation
        *self = *self + rhs;
    }
}

impl Sub<usize> for VAddr {
    type Output = VAddr;

    fn sub(self, rhs: usize) -> Self::Output {
        VAddr::new(self.0 - rhs)
    }
}

impl SubAssign<usize> for VAddr {
    fn sub_assign(&mut self, rhs: usize) {
        // XXX: this ensures correctness as it goes through the `Sub` implementation
        *self = *self - rhs;
    }
}
