use x86_64::{
    boot::MemoryObjectInfo,
    memory::{EntryFlags, PhysicalAddress, VirtualAddress},
};

pub struct MemoryObject {
    pub virtual_address: VirtualAddress,
    pub physical_address: PhysicalAddress,
    /// Number of 4KiB pages this memory object covers.
    // TODO: should this be in bytes instead?
    pub num_pages: usize,
    pub flags: EntryFlags,
}

impl MemoryObject {
    pub fn new(
        virtual_address: VirtualAddress,
        physical_address: PhysicalAddress,
        num_pages: usize,
        flags: EntryFlags,
    ) -> MemoryObject {
        MemoryObject { virtual_address, physical_address, num_pages, flags }
    }

    pub fn from_boot_info(memory_object_info: &MemoryObjectInfo) -> MemoryObject {
        MemoryObject {
            virtual_address: memory_object_info.virtual_address,
            physical_address: memory_object_info.physical_address,
            num_pages: memory_object_info.num_pages,
            flags: memory_object_info.permissions,
        }
    }
}
