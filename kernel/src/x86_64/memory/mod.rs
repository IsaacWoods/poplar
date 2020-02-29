//! This module contains the physical memory manager Pebble uses on x86_64.

mod buddy_allocator;
pub mod userspace_map;

use self::buddy_allocator::BuddyAllocator;
use core::ops::Range;
use spin::Mutex;
use x86_64::{
    boot::BootInfo,
    memory::{Frame, FrameAllocator},
};

const BUDDY_ALLOCATOR_MAX_ORDER: usize = 10;

/// The main physical memory manager. It tracks all conventional physical memory and is used by the
/// rest of the kernel to allocate physical memory.
struct PhysicalMemoryManager {
    /// A buddy allocator used to track all conventional memory. In the future, other allocators
    /// may be used to manage a subset of memory, such as memory for DMA.
    pub buddy_allocator: BuddyAllocator,
}

pub struct LockedPhysicalMemoryManager(Mutex<PhysicalMemoryManager>);

impl LockedPhysicalMemoryManager {
    pub fn new(boot_info: &BootInfo) -> LockedPhysicalMemoryManager {
        let mut buddy_allocator = BuddyAllocator::new(BUDDY_ALLOCATOR_MAX_ORDER);

        for entry in boot_info.memory_map.entries() {
            if entry.memory_type == boot_info_x86_64::MemoryType::Conventional {
                buddy_allocator.add_range(entry.frame_range());
            }
        }

        LockedPhysicalMemoryManager(Mutex::new(PhysicalMemoryManager { buddy_allocator }))
    }
}

impl FrameAllocator for LockedPhysicalMemoryManager {
    fn allocate_n(&self, n: usize) -> Range<Frame> {
        let start = self.0.lock().buddy_allocator.allocate_n(n).expect("Failed to allocate physical memory");
        start..(start + n)
    }

    fn free_n(&self, start: Frame, n: usize) {
        self.0.lock().buddy_allocator.free_n(start, n);
    }
}
