use core::{marker::PhantomData, slice, str};
use hal::{
    bootinfo::{Header, MemoryEntry, MemoryType},
    mem::{PAddr, PageTable, PageTableAllocator, VAddr},
};
use tracing::debug;

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

    pub fn header(&self) -> &Header {
        unsafe { &*self.base }
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

    pub fn memory_map_mut(&mut self) -> &mut [MemoryEntry] {
        let header = unsafe { *self.base };
        unsafe {
            slice::from_raw_parts_mut(
                self.base.byte_add(header.mem_map_offset as usize) as *mut MemoryEntry,
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

/// A frame allocator that directly allocates physical memory from the `BootInfo` memory map. This
/// is designed to be used early in the booting process before robust physical memory management is
/// running.
pub struct BootFrameAllocator<'a> {
    boot_info: &'a mut BootInfo,
    _phantom: PhantomData<*mut [MemoryEntry]>,
}

impl BootFrameAllocator<'_> {
    pub fn new<'a>(boot_info: &'a mut BootInfo) -> BootFrameAllocator<'a> {
        BootFrameAllocator {
            boot_info,
            _phantom: PhantomData,
        }
    }

    pub fn allocate(&self, frames: usize) -> PAddr {
        /*
         * We manually construct the mutable slice of the memory map so this can take a receive
         * a `&self`. This is sound because we hold an exclusive reference to the `BootInfo` and
         * can only be used from the context it was constructed in due to the `PhantomData`.
         */
        let header = unsafe { *self.boot_info.base };
        let memory_map = unsafe {
            slice::from_raw_parts_mut(
                self.boot_info.base.byte_add(header.mem_map_offset as usize) as *mut MemoryEntry,
                header.mem_map_length as usize,
            )
        };

        for i in 0..memory_map.len() {
            let entry = memory_map[i];
            if entry.typ == MemoryType::Usable
                && entry.length as usize >= frames * PageTable::PAGE_SIZE_4KIB
            {
                let base = PAddr::new(
                    entry.base as usize + entry.length as usize
                        - frames * PageTable::PAGE_SIZE_4KIB,
                );
                memory_map[i].length -= (frames * PageTable::PAGE_SIZE_4KIB) as u64;
                return base;
            }
        }

        panic!("Failed to allocate from boot allocator!");
    }
}

impl PageTableAllocator for BootFrameAllocator<'_> {
    fn alloc(&self) -> PAddr {
        self.allocate(1)
    }

    fn free(&self, frame: PAddr) {
        debug!(
            "Frame previously allocated from BootFrameAllocator freed; this frame has been leaked ({:#x})",
            frame
        );
    }
}
