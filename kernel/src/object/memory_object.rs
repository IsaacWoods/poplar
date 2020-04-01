use super::{alloc_kernel_object_id, KernelObject, KernelObjectId};
use alloc::sync::Arc;
use hal::{
    boot_info::Segment,
    memory::{Flags, PhysicalAddress, VirtualAddress},
};

pub struct MemoryObject {
    pub id: KernelObjectId,
    pub owner: KernelObjectId,
    pub virtual_address: VirtualAddress,
    pub physical_address: PhysicalAddress,
    /// Size of this MemoryObject in bytes.
    pub size: usize,
    pub flags: Flags,
}

impl MemoryObject {
    pub fn new(
        owner: KernelObjectId,
        virtual_address: VirtualAddress,
        physical_address: PhysicalAddress,
        size: usize,
        flags: Flags,
    ) -> Arc<MemoryObject> {
        Arc::new(MemoryObject {
            id: alloc_kernel_object_id(),
            owner,
            virtual_address,
            physical_address,
            size,
            flags,
        })
    }

    pub fn from_boot_info(owner: KernelObjectId, segment: &Segment, user_accessible: bool) -> Arc<MemoryObject> {
        Arc::new(MemoryObject {
            id: alloc_kernel_object_id(),
            owner,
            virtual_address: segment.virtual_address,
            physical_address: segment.physical_address,
            size: segment.size,
            flags: segment.flags,
        })
    }
}

impl KernelObject for MemoryObject {
    fn id(&self) -> KernelObjectId {
        self.id
    }
}
