mod buddy_allocator;

use crate::{object::task::TaskStack, per_cpu::KernelPerCpu};
use buddy_allocator::BuddyAllocator;
use core::{marker::PhantomData, ops::Range};
use hal::{
    boot_info::BootInfo,
    memory::{Frame, FrameAllocator, FrameSize, PhysicalAddress},
    Hal,
};
use spin::Mutex;

pub struct PhysicalMemoryManager<H> {
    buddy: Mutex<BuddyAllocator>,
    _phantom: PhantomData<H>,
}

impl<H> PhysicalMemoryManager<H>
where
    H: Hal<KernelPerCpu>,
{
    pub fn new(boot_info: &BootInfo) -> PhysicalMemoryManager<H> {
        let mut buddy_allocator = BuddyAllocator::new();

        for entry in boot_info.memory_map.entries() {
            if entry.memory_type == hal::boot_info::MemoryType::Conventional {
                buddy_allocator.add_range(entry.frame_range());
            }
        }

        PhysicalMemoryManager { buddy: Mutex::new(buddy_allocator), _phantom: PhantomData }
    }

    /// TODO: not sure this is the best interface to provide
    pub fn alloc_bytes(&self, num_bytes: usize) -> PhysicalAddress {
        /*
         * For now, we always use the buddy allocator.
         */
        self.buddy.lock().allocate_n(num_bytes).expect("Failed to allocate physical memory!")
    }
}

impl<H, S> FrameAllocator<S> for PhysicalMemoryManager<H>
where
    H: Hal<KernelPerCpu>,
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
