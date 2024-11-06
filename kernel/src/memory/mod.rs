mod buddy_allocator;
mod kernel_stack_allocator;
mod slab_allocator;

pub use kernel_stack_allocator::KernelStackAllocator;
pub use slab_allocator::SlabAllocator;

use buddy_allocator::BuddyAllocator;
use core::ops::Range;
use hal::memory::{Bytes, Frame, FrameAllocator, FrameSize, PAddr, VAddr};
use seed::boot_info::BootInfo;
use spinning_top::Spinlock;

pub struct PhysicalMemoryManager {
    buddy: Spinlock<BuddyAllocator>,
    pub kernel_stacks: KernelStackAllocator,
}

impl PhysicalMemoryManager {
    pub fn new(boot_info: &BootInfo, kernel_stack_allocator: KernelStackAllocator) -> PhysicalMemoryManager {
        let mut buddy_allocator = BuddyAllocator::new();

        for entry in &boot_info.memory_map {
            if entry.typ == seed::boot_info::MemoryType::Conventional {
                buddy_allocator.add_range(entry.frame_range());
            }
        }

        PhysicalMemoryManager { buddy: Spinlock::new(buddy_allocator), kernel_stacks: kernel_stack_allocator }
    }

    pub fn alloc_bytes(&self, num_bytes: Bytes) -> PAddr {
        /*
         * TODO: the whole physical memory management system needs a big overhaul now that the
         * kernel is significantly more complex:
         *    - We currently can only allocate whole blocks from the buddy allocator. This is not
         *      ideal for larger allocations that don't happen to be power-of-2 sized. We should
         *      investigate what better kernels do, but I'm guessing there should be layer(s) above
         *      the underlying buddy allocator.
         *    - The way we track allocations is very ad-hoc. We should return a structure with e.g.
         *      the length of the allocation, not just the allocated address and hope for the best.
         *    - We have no support for scatter-gather allocations, and instead insist that every
         *      allocation is contiguous, which is a restriction that very few usecases require.
         *      Especially for larger allocations, this would be a good improvement.
         */
        let num_bytes = num_bytes.next_power_of_two();
        self.buddy.lock().allocate_bytes(num_bytes).expect("Failed to allocate physical memory!")
    }
}

impl<S> FrameAllocator<S> for PhysicalMemoryManager
where
    S: FrameSize,
{
    fn allocate_n(&self, n: usize) -> Range<Frame<S>> {
        let start = self.buddy.lock().allocate_bytes(n * S::SIZE).expect("Failed to allocate physical memory!");
        Frame::<S>::starts_with(start)..(Frame::<S>::starts_with(start) + n)
    }

    fn free_n(&self, start: Frame<S>, num_frames: usize) {
        self.buddy.lock().free_bytes(start.start, num_frames * S::SIZE);
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
