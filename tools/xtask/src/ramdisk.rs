use crate::config::Platform;
use std::{
    fs::File,
    io::Write,
    mem,
    path::{Path, PathBuf},
};

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

pub struct Ramdisk {
    entries: Vec<Entry>,
    total_entries_size: u32,
    platform: Platform,
}

pub struct Entry {
    name: String,
    offset: u32,
    size: u32,
    source_file: PathBuf,
}

impl Ramdisk {
    pub fn new(platform: Platform) -> Ramdisk {
        Ramdisk { entries: Vec::new(), total_entries_size: 0, platform }
    }

    pub fn add(&mut self, name: &str, source: &Path) {
        let file_size = File::open(source).unwrap().metadata().unwrap().len() as u32;
        self.entries.push(Entry {
            name: name.to_string(),
            offset: self.total_entries_size,
            size: file_size,
            source_file: source.to_owned(),
        });
        self.total_entries_size += file_size;
    }

    /// Creates a file that contains the ramdisk header and entry table. This must be loaded at the
    /// ramdisk's base address using whichever mechanism required for the target platform.
    ///
    /// Create the ramdisk, returning a file that contains the contents of the header and entry
    /// table, and an iterator of entries in the form `(offset to load at, path to file to load)`.
    /// The mechanisms for loading these files into device memory is the responsibility of the
    /// caller, as it depends on the target platform.
    pub fn create(&self) -> (PathBuf, impl Iterator<Item = (u32, &Path)>) {
        let entries: Vec<RamdiskEntry> = self
            .entries
            .iter()
            .map(|entry| RamdiskEntry::new(&entry.name, entry.offset, entry.size).unwrap())
            .collect();

        let num_entries = entries.len() as u32;
        let header_size =
            mem::size_of::<RamdiskHeader>() as u32 + mem::size_of::<RamdiskEntry>() as u32 * num_entries;
        let header = RamdiskHeader {
            magic: RamdiskHeader::MAGIC,
            size: header_size + self.total_entries_size,
            num_entries,
        };

        let header_path = PathBuf::from(format!("ramdisk_header_{}.bin", self.platform));
        let mut file = File::create(&header_path).unwrap();
        let bytes = unsafe {
            std::slice::from_raw_parts(&header as *const _ as *const u8, mem::size_of::<RamdiskHeader>())
        };
        file.write_all(bytes).unwrap();
        file.write_all(unsafe {
            std::slice::from_raw_parts(
                entries.as_ptr() as *const u8,
                entries.len() * mem::size_of::<RamdiskEntry>(),
            )
        })
        .unwrap();

        (
            header_path,
            self.entries.iter().map(move |entry| (header_size + entry.offset, entry.source_file.as_path())),
        )
    }
}
