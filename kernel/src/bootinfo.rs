use core::{slice, str};
use hal::{
    bootinfo::{Header, MemoryEntry},
    mem::VAddr,
};

pub struct BootInfo {
    pub base: *const Header,
    pub string_table: *const u8,
    pub string_table_len: usize,
}

impl BootInfo {
    pub fn new(address: VAddr) -> BootInfo {
        let base = address.ptr() as *const Header;

        if unsafe { *base }.magic != hal::bootinfo::MAGIC {
            panic!("Boot info magic is invalid!");
        }

        let string_table =
            unsafe { base.byte_add((*base).string_table_offset as usize) } as *const u8;
        let string_table_len = unsafe { *base }.string_table_length as usize;

        BootInfo {
            base,
            string_table,
            string_table_len,
        }
    }

    pub fn memory_map(&self) -> &[MemoryEntry] {
        let header = unsafe { *self.base };
        unsafe {
            slice::from_raw_parts(
                self.base.byte_add(header.mem_map_offset as usize) as *const MemoryEntry,
                header.mem_map_length as usize,
            )
        }
    }

    pub fn read_string(&self, offset: u16, len: u16) -> &'_ str {
        assert!(
            offset as usize + len as usize <= self.string_table_len,
            "String access out of bounds!"
        );
        let ptr = unsafe { self.string_table.byte_add(offset as usize) } as *const u8;
        unsafe { str::from_raw_parts(ptr, len as usize) }
    }

    pub fn cmdline(&self) -> &'_ str {
        let header = unsafe { *self.base };
        self.read_string(header.cmdline_offset, header.cmdline_len)
    }
}
