//! This module contains the physical memory manager Pebble uses on x86_64.

mod buddy_allocator;
pub mod userspace_map;

use self::buddy_allocator::BuddyAllocator;
use core::ops::Range;
use hal::{
    boot_info::BootInfo,
    memory::{Flags, Frame, FrameAllocator, Mapper, Page, Size4KiB, VirtualAddress},
};
use hal_x86_64::kernel_map;
use pebble_util::bitmap::BitmapArray;
use spin::Mutex;

const BUDDY_ALLOCATOR_MAX_ORDER: usize = 10;

/// The main physical memory manager. It tracks all conventional physical memory and is used by the
/// rest of the kernel to allocate physical memory.
pub struct PhysicalMemoryManager {
    /// A buddy allocator used to track all conventional memory. In the future, other allocators
    /// may be used to manage a subset of memory, such as memory for DMA.
    pub buddy_allocator: BuddyAllocator,
    pub kernel_stack_bitmap: [u8; kernel_map::MAX_TASKS / 8],
}

#[derive(Clone, Copy, Debug)]
pub struct KernelStack {
    pub index: usize,
    pub top: VirtualAddress,
    pub slot_bottom: VirtualAddress,
    /// Depending on the initial size passed, not all of the slot may actually be mapped. This is the bottom of
    /// the actual mapped stack.
    pub stack_bottom: VirtualAddress,
}

pub struct LockedPhysicalMemoryManager(Mutex<PhysicalMemoryManager>);

impl LockedPhysicalMemoryManager {
    pub fn new(boot_info: &BootInfo) -> LockedPhysicalMemoryManager {
        let mut buddy_allocator = BuddyAllocator::new(BUDDY_ALLOCATOR_MAX_ORDER);

        for entry in boot_info.memory_map.entries() {
            if entry.memory_type == hal::boot_info::MemoryType::Conventional {
                buddy_allocator.add_range(entry.frame_range());
            }
        }

        LockedPhysicalMemoryManager(Mutex::new(PhysicalMemoryManager {
            buddy_allocator,
            kernel_stack_bitmap: [0; kernel_map::MAX_TASKS / 8],
        }))
    }

    pub fn get_kernel_stack(&self, initial_size: usize) -> Option<KernelStack> {
        if initial_size > kernel_map::STACK_SLOT_SIZE {
            panic!("Tried to make kernel stack that's larger than a kernel stack slot!");
        }

        let index = self.0.lock().kernel_stack_bitmap.alloc(1)?;
        let slot_bottom = kernel_map::KERNEL_STACKS_BASE + index * kernel_map::STACK_SLOT_SIZE;
        let top = slot_bottom + kernel_map::STACK_SLOT_SIZE - 1;
        let stack_bottom = top - initial_size;

        let pages = Page::contains(stack_bottom)..Page::contains(top + 1);
        let frames = self.allocate_n(pages.clone().count());
        crate::x86_64::ARCH
            .get()
            .kernel_page_table
            .lock()
            .mapper()
            .map_range(pages, frames, Flags { writable: true, ..Default::default() }, self)
            .unwrap();

        Some(KernelStack { index, top, slot_bottom, stack_bottom })
    }
}

impl FrameAllocator<Size4KiB> for LockedPhysicalMemoryManager {
    fn allocate_n(&self, n: usize) -> Range<Frame> {
        let start = self.0.lock().buddy_allocator.allocate_n(n).expect("Failed to allocate physical memory");
        start..(start + n)
    }

    fn free_n(&self, start: Frame, n: usize) {
        self.0.lock().buddy_allocator.free_n(start, n);
    }
}
