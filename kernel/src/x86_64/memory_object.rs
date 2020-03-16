use hal::{
    boot_info::Segment,
    memory::{Flags, PhysicalAddress, VirtualAddress},
};

pub struct MemoryObject {
    pub virtual_address: VirtualAddress,
    pub physical_address: PhysicalAddress,
    /// Size of this MemoryObject in bytes.
    pub size: usize,
    pub flags: Flags,
}

impl MemoryObject {
    pub fn new(
        virtual_address: VirtualAddress,
        physical_address: PhysicalAddress,
        size: usize,
        flags: Flags,
    ) -> MemoryObject {
        MemoryObject { virtual_address, physical_address, size, flags }
    }

    pub fn from_boot_info(segment: &Segment, user_accessible: bool) -> MemoryObject {
        MemoryObject {
            virtual_address: segment.virtual_address,
            physical_address: segment.physical_address,
            size: segment.size,
            flags: segment.flags,
        }
    }
}
