mod buddy;

use buddy::BuddyAllocator;
use core::ops::Range;
use hal::memory::{Frame, FrameAllocator, FrameSize, PAddr, Size4KiB};
use spinning_top::Spinlock;

/// The Physical Memory Manager (PMM) manages the system's supply of physical memory. It operates
/// in **frames** of 4KiB, which matches the base frame size on the architectures we're interested
/// in.
pub struct Pmm {
    buddy: Spinlock<BuddyAllocator>,
}

impl Pmm {
    pub fn new(memory_map: &[seed_bootinfo::MemoryEntry]) -> Pmm {
        let mut buddy_allocator = BuddyAllocator::new();

        for entry in memory_map {
            if entry.typ == seed_bootinfo::MemoryType::Usable {
                let frames = Frame::starts_with(PAddr::new(entry.base as usize).unwrap())
                    ..Frame::starts_with(PAddr::new(entry.base as usize + entry.length as usize).unwrap());
                buddy_allocator.free_range(frames);
            }
        }

        Pmm { buddy: Spinlock::new(buddy_allocator) }
    }

    /// Allocate `count` frames.
    pub fn alloc(&self, count: usize) -> PAddr {
        self.buddy.lock().alloc(count).expect("Failed to allocate requested physical memory")
    }

    /// Free `count` frames, starting at address `base`.
    pub fn free(&self, base: PAddr, count: usize) {
        self.buddy.lock().free(base, count)
    }
}

impl<S> FrameAllocator<S> for Pmm
where
    S: FrameSize,
{
    fn allocate_n(&self, n: usize) -> Range<Frame<S>> {
        let start =
            self.buddy.lock().alloc(n * S::SIZE / Size4KiB::SIZE).expect("Failed to allocate physical memory!");
        Frame::<S>::starts_with(start)..(Frame::<S>::starts_with(start) + n)
    }

    fn free_n(&self, start: Frame<S>, num_frames: usize) {
        self.buddy.lock().free(start.start, num_frames * S::SIZE / Size4KiB::SIZE);
    }
}
