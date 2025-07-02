#![no_std]

use core::{ffi::CStr, fmt};

#[repr(C)]
pub struct RamdiskHeader {
    pub magic: [u8; 8],
    /// The total size of the ramdisk, including this header, the entry table, and all of the
    /// entries.
    pub size: u32,
    pub num_entries: u32,
}

impl RamdiskHeader {
    pub const MAGIC: [u8; 8] = *b"RAMDISK_";
}

/// Describes a file held in the ramdisk.
#[repr(C)]
pub struct RamdiskEntry {
    /// The UTF-8 encoded name of the file. Must be null-terminated.
    pub name: [u8; Self::NAME_LENGTH],
    pub offset: u32,
    pub size: u32,
}

impl RamdiskEntry {
    pub const NAME_LENGTH: usize = 32;

    pub fn new(name: &str, offset: u32, size: u32) -> Result<RamdiskEntry, ()> {
        if name.as_bytes().len() > (Self::NAME_LENGTH - 1) {
            return Err(());
        }

        let mut name_bytes = [0; Self::NAME_LENGTH];
        name_bytes[..name.as_bytes().len()].copy_from_slice(name.as_bytes());
        Ok(RamdiskEntry { name: name_bytes, offset, size })
    }

    pub fn name(&self) -> Result<&str, ()> {
        CStr::from_bytes_until_nul(&self.name).map_err(|_| ())?.to_str().map_err(|_| ())
    }
}

impl fmt::Debug for RamdiskEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RamdiskEntry")
            .field("name", &self.name())
            .field("offset", &self.offset)
            .field("size", &self.size)
            .finish()
    }
}
