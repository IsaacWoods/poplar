use bnb::BitOps;
use core::{fmt, ops};

/// Represents a **valid** physical address. For most x86_64 platforms, physical addresses must be
/// less than `2^52`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct PAddr(usize);

impl PAddr {
    const MAX: PAddr = PAddr((1 << 52) - 1);

    /// Construct a new `PAddr`. Will panic if the provided value is not a valid address.
    pub const fn new(address: usize) -> PAddr {
        assert!(
            address <= Self::MAX.0,
            "Attempted to construct invalid physical address!"
        );
        PAddr(address)
    }

    /// Attempt to construct a new `PAddr`, returning `None` if the value is not a valid address.
    pub const fn try_new(address: usize) -> Option<PAddr> {
        if address <= Self::MAX.0 {
            Some(PAddr(address))
        } else {
            None
        }
    }

    /// Align this address to the given alignment, moving downwards if this is not already aligned.
    /// `align` must be `0` or a power-of-two.
    pub fn align_down(self, align: usize) -> PAddr {
        PAddr(crate::align_down(self.0, align))
    }

    pub fn align_up(self, align: usize) -> PAddr {
        PAddr(self.0 + align - 1).align_down(align)
    }

    pub fn is_aligned(self, align: usize) -> bool {
        self.0 % align == 0
    }

    pub fn checked_add(self, rhs: usize) -> Option<Self> {
        PAddr::try_new(self.0.checked_add(rhs)?)
    }

    pub fn checked_sub(self, rhs: usize) -> Option<Self> {
        PAddr::try_new(self.0.checked_sub(rhs)?)
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

const impl From<PAddr> for usize {
    fn from(address: PAddr) -> usize {
        address.0
    }
}

impl ops::Add<usize> for PAddr {
    type Output = PAddr;

    fn add(self, rhs: usize) -> Self::Output {
        match PAddr::try_new(self.0 + rhs) {
            Some(address) => address,
            None => panic!(
                "Physical address arithmetic led to invalid address: {:#x} + {:#x}",
                self, rhs
            ),
        }
    }
}

impl ops::AddAssign<usize> for PAddr {
    fn add_assign(&mut self, rhs: usize) {
        // Ensures correctness by going through the `Add` implementation
        *self = *self + rhs;
    }
}

impl ops::Sub<usize> for PAddr {
    type Output = PAddr;

    fn sub(self, rhs: usize) -> Self::Output {
        match PAddr::try_new(self.0 - rhs) {
            Some(address) => address,
            None => panic!(
                "Physical address arithmetic led to invalid address: {:#x} - {:#x}",
                self, rhs
            ),
        }
    }
}

impl ops::SubAssign<usize> for PAddr {
    fn sub_assign(&mut self, rhs: usize) {
        // Ensures correctness by going through the `Sub` implementation
        *self = *self - rhs;
    }
}

/// Represents a **canonical** virtual address.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct VAddr(usize);

impl VAddr {
    pub const fn new(address: usize) -> VAddr {
        VAddr(address).canonicalise()
    }

    pub const fn canonicalise(self) -> VAddr {
        const SIGN_EXTENSION: usize = 0o177777_000_000_000_000_0000;
        VAddr((SIGN_EXTENSION * ((self.0 >> 47) & 0b1)) | (self.0 & ((1 << 48) - 1)))
    }

    pub fn from_indices(p4: usize, p3: usize, p2: usize, p1: usize, offset: usize) -> VAddr {
        let mut addr = 0;
        addr.set_bits(0..12, offset);
        addr.set_bits(12..21, p1);
        addr.set_bits(21..30, p2);
        addr.set_bits(30..39, p3);
        addr.set_bits(39..48, p4);
        VAddr::new(addr)
    }

    pub const fn ptr<T>(self) -> *const T {
        self.0 as *const T
    }

    pub const fn mut_ptr<T>(self) -> *mut T {
        self.0 as *mut T
    }

    /// Align this address to the given alignment, moving downwards if this is not already aligned. `align` must
    /// be `0` or a power-of-two.
    pub fn align_down(self, align: usize) -> VAddr {
        VAddr(crate::align_down(self.0, align)).canonicalise()
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

    pub fn p4_index(&self) -> usize {
        self.0.bits(39..48)
    }
    pub fn p3_index(&self) -> usize {
        self.0.bits(30..39)
    }
    pub fn p2_index(&self) -> usize {
        self.0.bits(21..30)
    }
    pub fn p1_index(&self) -> usize {
        self.0.bits(12..21)
    }
    pub fn p_offset(&self) -> usize {
        self.0.bits(0..12)
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

const impl From<VAddr> for usize {
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

impl ops::Add<usize> for VAddr {
    type Output = VAddr;

    fn add(self, rhs: usize) -> Self::Output {
        VAddr::new(self.0 + rhs)
    }
}

impl ops::AddAssign<usize> for VAddr {
    fn add_assign(&mut self, rhs: usize) {
        // Ensures correctness by going through the `Add` implementation
        *self = *self + rhs;
    }
}

impl ops::Sub<usize> for VAddr {
    type Output = VAddr;

    fn sub(self, rhs: usize) -> Self::Output {
        VAddr::new(self.0 - rhs)
    }
}

impl ops::SubAssign<usize> for VAddr {
    fn sub_assign(&mut self, rhs: usize) {
        // Ensures correctness by going through the `Sub` implementation
        *self = *self - rhs;
    }
}
