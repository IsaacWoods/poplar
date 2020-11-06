mod buddy_allocator;
mod kernel_stack_allocator;
mod slab_allocator;

pub use kernel_stack_allocator::KernelStackAllocator;
pub use slab_allocator::SlabAllocator;

use buddy_allocator::BuddyAllocator;
use core::ops::Range;
use hal::{
    boot_info::BootInfo,
    memory::{Frame, FrameAllocator, FrameSize, PhysicalAddress, VirtualAddress},
};
use spin::Mutex;

pub struct PhysicalMemoryManager {
    buddy: Mutex<BuddyAllocator>,
}

impl PhysicalMemoryManager {
    pub fn new(boot_info: &BootInfo) -> PhysicalMemoryManager {
        let mut buddy_allocator = BuddyAllocator::new();

        for entry in boot_info.memory_map.entries() {
            if entry.memory_type == hal::boot_info::MemoryType::Conventional {
                buddy_allocator.add_range(entry.frame_range());
            }
        }

        PhysicalMemoryManager { buddy: Mutex::new(buddy_allocator) }
    }

    /// TODO: not sure this is the best interface to provide
    pub fn alloc_bytes(&self, num_bytes: usize) -> PhysicalAddress {
        /*
         * For now, we always use the buddy allocator.
         */
        self.buddy.lock().allocate_n(num_bytes).expect("Failed to allocate physical memory!")
    }
}

impl<S> FrameAllocator<S> for PhysicalMemoryManager
where
    S: FrameSize,
{
    fn allocate_n(&self, n: usize) -> Range<Frame<S>> {
        let start = self.buddy.lock().allocate_n(n * S::SIZE).expect("Failed to allocate physical memory!");
        Frame::<S>::starts_with(start)..(Frame::<S>::starts_with(start) + n)
    }

    fn free_n(&self, start: Frame<S>, num_frames: usize) {
        self.buddy.lock().free_n(start.start, num_frames * S::SIZE);
    }
}

/// Represents a stack, either in kernel-space or user-space. Stacks are allocated in "slots" of fixed size, but
/// only a subset of the slot may be mapped initially (to reduce physical memory usage). Stacks can't grow above
/// the size of their slot.
#[derive(Clone, Debug)]
pub struct Stack {
    pub top: VirtualAddress,
    pub slot_bottom: VirtualAddress,
    pub stack_bottom: VirtualAddress,

    pub physical_start: PhysicalAddress,
}
