mod buddy_allocator;

use buddy_allocator::BuddyAllocator;
use core::ops::Range;
use hal::{
    boot_info::BootInfo,
    memory::{Frame, FrameAllocator, Size4KiB},
};
use spin::Mutex;

struct PhysicalMemoryManager {
    buddy_allocator: BuddyAllocator,
}

pub struct LockedPhysicalMemoryManager(Mutex<PhysicalMemoryManager>);

impl LockedPhysicalMemoryManager {
    pub fn new(boot_info: &BootInfo) -> LockedPhysicalMemoryManager {
        let mut buddy_allocator = BuddyAllocator::new();

        for entry in boot_info.memory_map.entries() {
            if entry.memory_type == hal::boot_info::MemoryType::Conventional {
                buddy_allocator.add_range(entry.frame_range());
            }
        }

        LockedPhysicalMemoryManager(Mutex::new(PhysicalMemoryManager { buddy_allocator }))
    }
}

impl FrameAllocator<Size4KiB> for LockedPhysicalMemoryManager {
    fn allocate_n(&self, n: usize) -> Range<Frame<Size4KiB>> {
        let start = self.0.lock().buddy_allocator.allocate_n(n).expect("Failed to allocate physical memory");
        start..(start + n)
    }

    fn free_n(&self, start: Frame, n: usize) {
        self.0.lock().buddy_allocator.free_n(start, n);
    }
}
