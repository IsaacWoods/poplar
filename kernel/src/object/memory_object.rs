use super::{alloc_kernel_object_id, KernelObject, KernelObjectId};
use alloc::sync::Arc;
use hal::{
    boot_info::Segment,
    memory::{Flags, PAddr, VAddr},
};

pub struct MemoryObject {
    pub id: KernelObjectId,
    pub owner: KernelObjectId,
    /// The virtual address to map this MemoryObject at. If this is `None`, the mapping task can choose to map it
    /// at any virtual address it chooses.
    pub virtual_address: Option<VAddr>,
    pub physical_address: PAddr,
    /// Size of this MemoryObject in bytes.
    pub size: usize,
    pub flags: Flags,
}

impl MemoryObject {
    pub fn new(
        owner: KernelObjectId,
        virtual_address: Option<VAddr>,
        physical_address: PAddr,
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

    pub fn from_boot_info(owner: KernelObjectId, segment: &Segment) -> Arc<MemoryObject> {
        Arc::new(MemoryObject {
            id: alloc_kernel_object_id(),
            owner,
            virtual_address: Some(segment.virtual_address),
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
