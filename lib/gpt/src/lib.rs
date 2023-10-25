#![no_std]
#![feature(const_option)]

mod guid;

pub use guid::Guid;

use core::ffi::CStr;

#[derive(Clone, Debug)]
#[repr(C)]
pub struct GptHeader {
    pub signature: [u8; 8],
    pub revision: u32,
    pub header_size: u32,
    pub header_crc: u32,
    _reserved0: u32,
    pub my_lba: u64,
    pub alternate_lba: u64,
    pub first_usable_lba: u64,
    pub last_usable_lba: u64,
    pub disk_guid: Guid,
    pub partition_entry_lba: u64,
    pub num_partition_entries: u32,
    pub size_of_partition_entry: u32,
    pub partition_entry_array_crc: u32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HeaderError {
    InvalidSignature,
    InvalidCrc,
}

impl GptHeader {
    pub fn validate(&self) -> Result<(), HeaderError> {
        if self.signature != *b"EFI PART" {
            return Err(HeaderError::InvalidSignature);
        }

        // TODO: validate crc

        Ok(())
    }
}

#[derive(Clone, Debug)]
#[repr(C)]
pub struct PartitionEntry {
    pub partition_type_guid: Guid,
    pub unique_partition_guid: Guid,
    pub starting_lba: u64,
    pub ending_lba: u64,
    pub attributes: PartitionAttributes,
    pub partition_name: [u8; 72],
}

impl PartitionEntry {
    pub fn name(&self) -> Result<&str, ()> {
        CStr::from_bytes_until_nul(&self.partition_name).map_err(|_| ())?.to_str().map_err(|_| ())
    }
}

#[derive(Clone, Debug)]
#[repr(transparent)]
pub struct PartitionAttributes(u64);
