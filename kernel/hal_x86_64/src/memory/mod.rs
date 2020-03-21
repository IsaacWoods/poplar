mod buddy_allocator;

use buddy_allocator::BuddyAllocator;
use core::ops::Range;
use hal::memory::{Frame, FrameAllocator, Size4KiB};
use spin::Mutex;

struct PhysicalMemoryManager {
    buddy_allocator: BuddyAllocator,
}

pub struct LockedPhysicalMemoryManager(Mutex<PhysicalMemoryManager>);

impl LockedPhysicalMemoryManager {}

impl FrameAllocator<Size4KiB> for LockedPhysicalMemoryManager {
    fn allocate_n(&self, n: usize) -> Range<Frame<Size4KiB>> {
        let start = self.0.lock().buddy_allocator.allocate_n(n).expect("Failed to allocate physical memory");
        start..(start + n)
    }

    fn free_n(&self, start: Frame, n: usize) {
        self.0.lock().buddy_allocator.free_n(start, n);
    }
}
