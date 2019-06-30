pub mod boot_services;
pub mod protocols;
pub mod runtime_services;
pub mod system_table;

use self::system_table::SystemTable;
use core::{fmt, ops, ptr::Unique};

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

const ERROR_BIT: usize = 0x8000_0000_0000_0000;

/// Status code
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(usize)]
pub enum Status {
    Success = 0,
    LoadError = ERROR_BIT | 1,
    InvalidParameter = ERROR_BIT | 2,
    Unsupported = ERROR_BIT | 3,
    BadBufferSize = ERROR_BIT | 4,
    BufferTooSmall = ERROR_BIT | 5,
    NotReady = ERROR_BIT | 6,
    DeviceError = ERROR_BIT | 7,
    WriteProtected = ERROR_BIT | 8,
    OutOfResources = ERROR_BIT | 9,
    VolumeCorrupted = ERROR_BIT | 10,
    VolumeFull = ERROR_BIT | 11,
    NoMedia = ERROR_BIT | 12,
    MediaChanged = ERROR_BIT | 13,
    NotFound = ERROR_BIT | 14,
    AccessDenied = ERROR_BIT | 15,
    NoResponse = ERROR_BIT | 16,
    NoMapping = ERROR_BIT | 17,
    Timeout = ERROR_BIT | 18,
    NotStarted = ERROR_BIT | 19,
    AlreadyStarted = ERROR_BIT | 20,
    Aborted = ERROR_BIT | 21,
    IcmpError = ERROR_BIT | 22,
    TftpError = ERROR_BIT | 23,
    ProtocolError = ERROR_BIT | 24,
    IncompatibleVersion = ERROR_BIT | 25,
    SecurityViolation = ERROR_BIT | 26,
    CrcError = ERROR_BIT | 27,
    EndOfMedia = ERROR_BIT | 28,
    EndOfFile = ERROR_BIT | 31,
    InvalidLanguage = ERROR_BIT | 32,
    CompromisedData = ERROR_BIT | 33,
    IpAddressConflict = ERROR_BIT | 34,
    HttpError = ERROR_BIT | 35,
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
        if (*self as usize) & ERROR_BIT != 0 {
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

/// Returns a reference to the `SystemTable`. This is safe to call after the global has been
/// initialised, which we do straight after control is passed to us.
pub fn system_table() -> &'static SystemTable {
    unsafe { &*SYSTEM_TABLE }
}

pub fn image_handle() -> Handle {
    unsafe { IMAGE_HANDLE }
}

/*
 * It's only safe to have these `static mut`s because we know the bootloader will only have one
 * thread of execution and is completely non-reentrant.
 */
static mut SYSTEM_TABLE: *const SystemTable = 0 as *const _;
static mut IMAGE_HANDLE: Handle = 0;

pub unsafe fn init(system_table: *const SystemTable, image_handle: Handle) {
    SYSTEM_TABLE = system_table;
    IMAGE_HANDLE = image_handle;
}
