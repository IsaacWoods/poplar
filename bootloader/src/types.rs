use core::{convert, fmt, ops, ptr::Unique};

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

/// Pointer to EFI runtime memory
///
/// An RuntimeMemory is a read-only pointer to something in EFI "runtime memory". According to the
/// UEFI specification, the operating system must never overwrite or deallocate runtime memory, so
/// this pointer is always safe to dereference (assuming runtime memory is mapped).
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(C)]
pub struct Guid {
    pub a: u32,
    pub b: u16,
    pub c: u16,
    pub d: [u8; 8],
}

impl fmt::Display for Guid {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let d = {
            let (low, high) = (u16::from(self.d[0]), u16::from(self.d[1]));

            (low << 8) | high
        };

        // Reverse order of the bytes
        let e = self.d[2..8].iter().enumerate().fold(0, |acc, (i, &elem)| {
            acc | {
                let shift = (5 - i) * 8;
                u64::from(elem) << shift
            }
        });

        write!(fmt, "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}", self.a, self.b, self.c, d, e)
    }
}
