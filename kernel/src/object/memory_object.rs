use super::{alloc_kernel_object_id, KernelObject, KernelObjectId, KernelObjectType};
use alloc::{sync::Arc, vec::Vec};
use hal::memory::{Flags, PAddr};
use seed::boot_info::Segment;
use spinning_top::Spinlock;

#[derive(Debug)]
pub struct MemoryObject {
    pub id: KernelObjectId,
    pub owner: KernelObjectId,
    pub inner: Spinlock<Inner>,
}

#[derive(Debug)]
pub struct Inner {
    /// Size of this MemoryObject in bytes.
    pub size: usize,
    pub flags: Flags,
    pub backing: Vec<(PAddr, usize)>,
}

impl MemoryObject {
    pub fn new(owner: KernelObjectId, physical_address: PAddr, size: usize, flags: Flags) -> Arc<MemoryObject> {
        Arc::new(MemoryObject {
            id: alloc_kernel_object_id(),
            owner,
            inner: Spinlock::new(Inner { size, flags, backing: vec![(physical_address, size)] }),
        })
    }

    pub fn from_boot_info(owner: KernelObjectId, segment: &Segment) -> Arc<MemoryObject> {
        Arc::new(MemoryObject {
            id: alloc_kernel_object_id(),
            owner,
            inner: Spinlock::new(Inner {
                size: segment.size,
                flags: segment.flags,
                backing: vec![(segment.physical_address, segment.size)],
            }),
        })
    }

    /// Extend this `MemoryObject` by `extend_by` bytes. The new portion of the object is backed
    /// by physical memory starting at `new_backing`.
    ///
    /// ### Note
    /// Note that this does not map the new portion of the object into address spaces that this
    /// memory object is already mapped into.
    pub unsafe fn extend(&self, extend_by: usize, new_backing: PAddr) {
        assert!(extend_by > 0);
        let mut inner = self.inner.lock();
        inner.size += extend_by;
        inner.backing.push((new_backing, extend_by));
    }

    pub fn size(&self) -> usize {
        self.inner.lock().size
    }

    pub fn flags(&self) -> Flags {
        self.inner.lock().flags
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

impl PartialEq for MemoryObject {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
