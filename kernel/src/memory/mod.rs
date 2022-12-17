mod buddy_allocator;
mod kernel_stack_allocator;
mod slab_allocator;

pub use kernel_stack_allocator::KernelStackAllocator;
pub use slab_allocator::SlabAllocator;

use buddy_allocator::BuddyAllocator;
use core::ops::Range;
use hal::memory::{Frame, FrameAllocator, FrameSize, PAddr, VAddr};
use seed::boot_info::BootInfo;
use spin::Mutex;

pub struct PhysicalMemoryManager {
    buddy: Mutex<BuddyAllocator>,
}

impl PhysicalMemoryManager {
    pub fn new(boot_info: &BootInfo) -> PhysicalMemoryManager {
        let mut buddy_allocator = BuddyAllocator::new();

        for entry in &boot_info.memory_map {
            if entry.typ == seed::boot_info::MemoryType::Conventional {
                buddy_allocator.add_range(entry.frame_range());
            }
        }

        PhysicalMemoryManager { buddy: Mutex::new(buddy_allocator) }
    }

    pub fn alloc_bytes(&self, num_bytes: usize) -> PAddr {
        /*
         * For now, we always use the buddy allocator.
         * TODO: this isn't very good. We can only allocate a whole block at a time, and always allocate a
         * contiguous block of memory even when we don't need one. This should return an "owned" allocation that
         * can hold a list of non-contiguous ranges of frames, plus information needed to free the allocation.
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
    pub top: VAddr,
    pub slot_bottom: VAddr,
    pub stack_bottom: VAddr,

    pub physical_start: PAddr,
}
