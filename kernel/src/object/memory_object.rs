use super::{alloc_kernel_object_id, KernelObject, KernelObjectId, KernelObjectType};
use alloc::sync::Arc;
use hal::memory::{Flags, PAddr};
use seed::boot_info::Segment;

#[derive(Debug)]
pub struct MemoryObject {
    pub id: KernelObjectId,
    pub owner: KernelObjectId,
    pub physical_address: PAddr,
    /// Size of this MemoryObject in bytes.
    pub size: usize,
    pub flags: Flags,
}

impl MemoryObject {
    pub fn new(owner: KernelObjectId, physical_address: PAddr, size: usize, flags: Flags) -> Arc<MemoryObject> {
        Arc::new(MemoryObject { id: alloc_kernel_object_id(), owner, physical_address, size, flags })
    }

    pub fn from_boot_info(owner: KernelObjectId, segment: &Segment) -> Arc<MemoryObject> {
        Arc::new(MemoryObject {
            id: alloc_kernel_object_id(),
            owner,
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

    fn typ(&self) -> KernelObjectType {
        KernelObjectType::MemoryObject
    }
}
