//! This module contains the physical memory manager Pebble uses on x86_64.

mod buddy_allocator;

use self::buddy_allocator::BuddyAllocator;
use core::ops::Range;
use spin::Mutex;
use x86_64::boot::BootInfo;
use x86_64::memory::paging::{Frame, FrameAllocator};

const BUDDY_ALLOCATOR_MAX_ORDER: usize = 10;

pub struct MemoryController {
    pub buddy_allocator: BuddyAllocator,
}

impl MemoryController {
    pub fn new(boot_info: &BootInfo) -> MemoryController {
        let mut buddy_allocator = BuddyAllocator::new(BUDDY_ALLOCATOR_MAX_ORDER);

        for entry in boot_info.memory_entries() {
            if entry.memory_type == x86_64::boot::MemoryType::Conventional {
                buddy_allocator.add_range(entry.area.clone());
            }
        }

        MemoryController { buddy_allocator }
    }
}

pub struct LockedMemoryController(Mutex<MemoryController>);

impl LockedMemoryController {
    pub fn new(boot_info: &BootInfo) -> LockedMemoryController {
        LockedMemoryController(Mutex::new(MemoryController::new(boot_info)))
    }
}

impl FrameAllocator for LockedMemoryController {
    fn allocate_n(&self, n: usize) -> Range<Frame> {
        let start = self
            .0
            .lock()
            .buddy_allocator
            .allocate_n(n)
            .expect("Failed to allocate of physical memory");
        start..(start + n)
    }

    fn free_n(&self, start: Frame, n: usize) {
        self.0.lock().buddy_allocator.free_n(start, n);
    }
}
