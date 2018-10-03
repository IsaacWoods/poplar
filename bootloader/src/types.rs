use core::ptr;
use core::{convert, ops, ptr::Unique};

/// Logical boolean
#[derive(Debug)]
#[repr(u8)]
pub enum Bool {
    False = 0,
    True = 1,
}

impl convert::From<bool> for Bool {
    fn from(b: bool) -> Self {
        match b {
            false => Bool::False,
            true => Bool::True,
        }
    }
}

pub type Char16 = u16;

/// Pointer to EFI boot services memory
///
/// An BootMemory is a read-only pointer to something in EFI "boot services memory". According to the
/// UEFI specification, this memory is owned by boot-time EFI drivers and services, but may be
/// freely used/overwritten by the operating system after exiting boot services. As such, the
/// pointer may be freely dereferenced in a pre-boot environment but not after.
#[derive(Debug)]
#[repr(C)]
pub struct BootMemory<T>(Unique<T>);

impl<T> BootMemory<T> {
    /// Creates a null BootMemory pointer
    ///
    /// This method is primarily useful when dealing with foreign APIs that return pointers via out
    /// parameters, where the caller needs to have a mutable pointer available but the referent of
    /// that pointer is irrelevant since the API will overwrite the pointer.
    ///
    /// # Safety
    ///
    /// The caller is responsible for ensuring the pointer is set to something valid before it is
    /// dereferenced. The `is_null` method may be helpful in such validation.
    pub(crate) unsafe fn new() -> BootMemory<T> {
        BootMemory(Unique::new_unchecked(ptr::null_mut()))
    }

    /// Determines whether this BootMemory is null
    ///
    /// This method is useful for validating that an BootMemory value has been set to some value. Of
    /// course a non-null pointer may still refer to an invalid location, but this method can at
    /// least show whether a foreign API successfully changed the value of an BootMemory to something
    /// non-null.
    pub(crate) fn is_null(&self) -> bool {
        self.0.as_ptr().is_null()
    }
}

impl<T> ops::Deref for BootMemory<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { self.0.as_ref() }
    }
}

/// Pointer to EFI runtime memory
///
/// An RuntimeMemory is a read-only pointer to something in EFI "runtime memory". According to the UEFI
/// specification, the operating system must never overwrite or deallocate runtime memory, so this
/// pointer is always safe to dereference (assuming runtime memory is mapped).
#[derive(Debug)]
#[repr(C)]
pub struct RuntimeMemory<T>(Unique<T>);

impl<T> ops::Deref for RuntimeMemory<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { self.0.as_ref() }
    }
}

pub type Handle = usize;

/// Used to differentiate status codes
const HIGHBIT: usize = 0x8000_0000_0000_0000;

/// Status code
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(usize)]
pub enum Status {
    Success = 0,
    LoadError = HIGHBIT | 1,
    InvalidParameter = HIGHBIT | 2,
    Unsupported = HIGHBIT | 3,
    BadBufferSize = HIGHBIT | 4,
    BufferTooSmall = HIGHBIT | 5,
    NotReady = HIGHBIT | 6,
    DeviceError = HIGHBIT | 7,
    WriteProtected = HIGHBIT | 8,
    OutOfResources = HIGHBIT | 9,
    VolumeCorrupted = HIGHBIT | 10,
    VolumeFull = HIGHBIT | 11,
    NoMedia = HIGHBIT | 12,
    MediaChanged = HIGHBIT | 13,
    NotFound = HIGHBIT | 14,
    AccessDenied = HIGHBIT | 15,
    NoResponse = HIGHBIT | 16,
    NoMapping = HIGHBIT | 17,
    Timeout = HIGHBIT | 18,
    NotStarted = HIGHBIT | 19,
    AlreadyStarted = HIGHBIT | 20,
    Aborted = HIGHBIT | 21,
    IcmpError = HIGHBIT | 22,
    TftpError = HIGHBIT | 23,
    ProtocolError = HIGHBIT | 24,
    IncompatibleVersion = HIGHBIT | 25,
    SecurityViolation = HIGHBIT | 26,
    CrcError = HIGHBIT | 27,
    EndOfMedia = HIGHBIT | 28,
    EndOfFile = HIGHBIT | 31,
    InvalidLanguage = HIGHBIT | 32,
    CompromisedData = HIGHBIT | 33,
    IpAddressConflict = HIGHBIT | 34,
    HttpError = HIGHBIT | 35,
    WarnUnknownGlyph = 1,
    WarnDeleteFailure = 2,
    WarnWriteFailure = 3,
    WarnBufferTooSmall = 4,
    WarnStaleData = 5,
    WarnFileSystem = 6,
    WarnResetRequired = 7,
}

impl Status {
    /// Converts the status to a Result type
    ///
    /// According to the EFI specification, negative status codes are considered errors, and zero or
    /// above is considered success. However, even a successful status code might have include a
    /// useful warning, so it is preserved here in the Result's Ok variant.
    // TODO: maybe replace with `Try` impl
    pub fn as_result(&self) -> Result<Status, Status> {
        // If HIGHBIT is set, this is an error
        if (*self as usize) & HIGHBIT != 0 {
            Err(*self)
        } else {
            Ok(*self)
        }
    }
}

/// Data structure that precedes all of the standard EFI table types
#[derive(Debug)]
#[repr(C)]
pub struct TableHeader {
    pub signature: u64,
    pub revision: u32,
    pub header_size: u32,
    pub crc32: u32,
    pub reserved: u32,
}
