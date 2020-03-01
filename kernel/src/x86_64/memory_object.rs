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

    pub fn from_boot_info(memory_object_info: &Segment, user_accessible: bool) -> MemoryObject {
        let flags = EntryFlags::PRESENT
            | if memory_object_info.writable { EntryFlags::WRITABLE } else { EntryFlags::empty() }
            | if memory_object_info.executable { EntryFlags::empty() } else { EntryFlags::NO_EXECUTE }
            | if user_accessible { EntryFlags::USER_ACCESSIBLE } else { EntryFlags::empty() };

        MemoryObject {
            virtual_address: memory_object_info.virtual_address,
            physical_address: memory_object_info.physical_address,
            size: memory_object_info.size,
            flags,
        }
    }
}
