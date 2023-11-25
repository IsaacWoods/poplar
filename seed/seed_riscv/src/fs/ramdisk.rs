use super::{File, Filesystem};
use alloc::{slice, string::ToString};
use core::mem;
use hal::memory::PAddr;
use seed::ramdisk::{RamdiskEntry, RamdiskHeader};

pub struct Ramdisk {
    base: *const RamdiskHeader,
    offset_to_data: usize,
}

impl Ramdisk {
    pub unsafe fn new(address: usize) -> Option<Ramdisk> {
        if unsafe { *(address as *const [u8; 8]) } != RamdiskHeader::MAGIC {
            return None;
        }

        let header = unsafe { &*(address as *const RamdiskHeader) };
        let offset_to_data =
            mem::size_of::<RamdiskHeader>() + header.num_entries as usize * mem::size_of::<RamdiskEntry>();
        Some(Ramdisk { base: address as *const RamdiskHeader, offset_to_data })
    }

    pub fn entry(&self, name: &str) -> Option<&RamdiskEntry> {
        self.entries().into_iter().find(|entry| entry.name().unwrap() == name)
    }

    pub fn entry_data(&self, name: &str) -> Option<&[u8]> {
        let entry = self.entry(name)?;

        unsafe {
            Some(slice::from_raw_parts(
                self.base.byte_add(self.offset_to_data + entry.offset as usize) as *const u8,
                entry.size as usize,
            ))
        }
    }

    pub fn header(&self) -> &RamdiskHeader {
        unsafe { &*self.base }
    }

    pub fn entries(&self) -> &[RamdiskEntry] {
        let entries_base = unsafe { self.base.byte_add(mem::size_of::<RamdiskHeader>()) as *const RamdiskEntry };
        unsafe { slice::from_raw_parts(entries_base, self.header().num_entries as usize) }
    }

    /// Get the memory region occupied by the ramdisk, in the form `(address, size)`.
    pub fn memory_region(&self) -> (PAddr, usize) {
        (PAddr::new(self.base as usize).unwrap(), self.header().size as usize)
    }
}

impl Filesystem for Ramdisk {
    fn load(&mut self, path: &str) -> Result<super::File, ()> {
        self.entry_data(path).map(|data| File { path: path.to_string(), data }).ok_or(())
    }

    fn close(&mut self, _file: super::File) {}
}
