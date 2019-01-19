use super::buddy_allocator::BuddyAllocator;
use core::ops::Range;
use spin::Mutex;
use x86_64::{
    boot::BootInfo,
    memory::paging::{Frame, FrameAllocator},
};

const BUDDY_ALLOCATOR_MAX_ORDER: usize = 10;

/// The main physical memory manager. It tracks all conventional physical memory and is used by the
/// rest of the kernel to allocate physical memory.
pub struct PhysicalMemoryManager {
    /// A buddy allocator used to track all conventional memory. In the future, other allocators
    /// may be used to manage a subset of memory, such as memory for DMA.
    pub buddy_allocator: BuddyAllocator,
}

pub struct LockedPhysicalMemoryManager(Mutex<PhysicalMemoryManager>);

impl LockedPhysicalMemoryManager {
    pub fn new(boot_info: &BootInfo) -> LockedPhysicalMemoryManager {
        let mut buddy_allocator = BuddyAllocator::new(BUDDY_ALLOCATOR_MAX_ORDER);

        for entry in boot_info.memory_entries() {
            if entry.memory_type == x86_64::boot::MemoryType::Conventional {
                buddy_allocator.add_range(entry.area.clone());
            }
        }

        LockedPhysicalMemoryManager(Mutex::new(PhysicalMemoryManager { buddy_allocator }))
    }
}

impl FrameAllocator for LockedPhysicalMemoryManager {
    fn allocate_n(&self, n: usize) -> Range<Frame> {
        let start = self
            .0
            .lock()
            .buddy_allocator
            .allocate_n(n)
            .expect("Failed to allocate physical memory");
        start..(start + n)
    }

    fn free_n(&self, start: Frame, n: usize) {
        self.0.lock().buddy_allocator.free_n(start, n);
    }
}
