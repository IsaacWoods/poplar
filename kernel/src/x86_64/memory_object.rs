use x86_64::{
    boot::MemoryObjectInfo,
    memory::{EntryFlags, PhysicalAddress, VirtualAddress},
};

pub struct MemoryObject {
    pub virtual_address: VirtualAddress,
    pub physical_address: PhysicalAddress,
    /// Size of this MemoryObject in bytes.
    pub size: usize,
    pub flags: EntryFlags,
}

impl MemoryObject {
    pub fn new(
        virtual_address: VirtualAddress,
        physical_address: PhysicalAddress,
        size: usize,
        flags: EntryFlags,
    ) -> MemoryObject {
        MemoryObject { virtual_address, physical_address, size, flags }
    }

    pub fn from_boot_info(memory_object_info: &MemoryObjectInfo) -> MemoryObject {
        MemoryObject {
            virtual_address: memory_object_info.virtual_address,
            physical_address: memory_object_info.physical_address,
            size: memory_object_info.size,
            flags: memory_object_info.permissions,
        }
    }
}
