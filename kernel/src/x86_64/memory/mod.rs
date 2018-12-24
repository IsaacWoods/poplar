//! This module contains the physical memory manager Pebble uses on x86_64.

mod buddy_allocator;

use self::buddy_allocator::BuddyAllocator;
use core::ops::Range;
use core::borrow::BorrowMut;
use spin::{Mutex, MutexGuard};
use x86_64::boot::BootInfo;
use x86_64::memory::paging::{Frame, FrameAllocator, ActivePageTable};
use x86_64::memory::paging::table::RecursiveMapping;

const BUDDY_ALLOCATOR_MAX_ORDER: usize = 10;

pub struct MemoryController {
    pub buddy_allocator: BuddyAllocator,
    pub kernel_page_table: ActivePageTable<RecursiveMapping>,
}

impl MemoryController {
    pub fn new(boot_info: &BootInfo) -> MemoryController {
        let mut buddy_allocator = BuddyAllocator::new(BUDDY_ALLOCATOR_MAX_ORDER);

        for entry in boot_info.memory_entries() {
            if entry.memory_type == x86_64::boot::MemoryType::Conventional {
                buddy_allocator.add_range(entry.area.clone());
            }
        }

        MemoryController {
            buddy_allocator,

            /*
             * Here, we assume the bootloader has installed a set of recursively-mapped page tables
             * for the kernel. If this assumption is not true, very bad things will happen, so
             * unsafe.
             */
            kernel_page_table: unsafe { ActivePageTable::<RecursiveMapping>::new() },
        }
    }
}

pub struct LockedMemoryController(Mutex<MemoryController>);

impl LockedMemoryController {
    pub fn new(boot_info: &BootInfo) -> LockedMemoryController {
        LockedMemoryController(Mutex::new(MemoryController::new(boot_info)))
    }

    pub fn kernel_tables(&mut self) -> MutexGuard<ActivePageTable<RecursiveMapping>> {
        self.0.borrowing_lock(|controller| &mut controller.kernel_page_table)
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
