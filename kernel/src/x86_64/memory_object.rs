use x86_64::{
    boot::MemoryObjectInfo,
    memory::{EntryFlags, PhysicalAddress, VirtualAddress},
};

pub struct MemoryObject {
    pub virtual_address: VirtualAddress,
    pub physical_address: PhysicalAddress,
    /// Number of 4KiB pages this memory object covers.
    pub num_pages: usize,
    pub flags: EntryFlags,
}

impl MemoryObject {
    pub fn new(memory_object_info: &MemoryObjectInfo) -> MemoryObject {
        MemoryObject {
            virtual_address: memory_object_info.virtual_address,
            physical_address: memory_object_info.physical_address,
            num_pages: memory_object_info.num_pages,
            flags: memory_object_info.permissions,
        }
    }
}
