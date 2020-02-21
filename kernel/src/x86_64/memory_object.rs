use boot_info_x86_64::Segment;
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

    pub fn from_boot_info(memory_object_info: &Segment) -> MemoryObject {
        let flags = EntryFlags::PRESENT
            | if memory_object_info.writable { EntryFlags::WRITABLE } else { EntryFlags::empty() }
            | if memory_object_info.executable { EntryFlags::empty() } else { EntryFlags::NO_EXECUTE };

        MemoryObject {
            virtual_address: VirtualAddress::new(memory_object_info.virtual_address),
            physical_address: PhysicalAddress::new(memory_object_info.physical_address).unwrap(),
            size: memory_object_info.size,
            flags,
        }
    }
}
