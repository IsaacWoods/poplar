mod slab_allocator;

use crate::Platform;
use core::sync::atomic::{AtomicUsize, Ordering};
use hal::memory::{Flags, FrameSize, PAddr, PageTable, Size4KiB, VAddr};
use spinning_top::Spinlock;

pub struct Vmm<P: Platform> {
    pub kernel_page_table: Spinlock<P::PageTable>,
    // Tracks the next available address in the kernel dynamic allocation map
    // TODO: this should be replaced by some sort of tree
    kernel_next_available: AtomicUsize,
    kernel_end_of_dynamic_area: VAddr,
}

impl<P> Vmm<P>
where
    P: Platform,
{
    pub fn new(kernel_page_table: P::PageTable) -> Vmm<P> {
        Vmm {
            kernel_page_table: Spinlock::new(kernel_page_table),
            kernel_next_available: AtomicUsize::new(usize::from(P::KERNEL_DYNAMIC_AREA_BASE)),
            // TODO: I guess this could not always be the case? Maybe use another constant?
            kernel_end_of_dynamic_area: P::KERNEL_IMAGE_BASE,
        }
    }

    // TODO: maybe check the physical address is normal ram here??
    pub fn physical_to_virtual(&self, phys: PAddr) -> VAddr {
        P::PHYSICAL_MAPPING_BASE + usize::from(phys)
    }

    // TODO: this could return an owned object that frees the space when dropped
    pub fn alloc_kernel(&self, size: usize) -> Option<VAddr> {
        let size = mulch::math::align_up(size, Size4KiB::SIZE);
        let address = VAddr::new(self.kernel_next_available.fetch_add(size, Ordering::Relaxed));

        if (address + size) > self.kernel_end_of_dynamic_area {
            tracing::warn!("Failed to allocate space in kernel virtual dynamic area!");
            return None;
        }

        Some(address)
    }

    // TODO: this could return an owned object that frees the space when dropped
    /// Map an area of memory at the given physical address into the kernel dynamic memory area.
    pub fn map_kernel(&self, addr: PAddr, size: usize, flags: Flags) -> Option<VAddr> {
        let virt = self.alloc_kernel(size)?;
        self.kernel_page_table.lock().map_area(virt, addr, size, flags, crate::PMM.get()).unwrap();
        Some(virt)
    }

    pub fn alloc_kernel_stack(&self, initial_size: usize) -> Option<Stack> {
        use hal::memory::{Flags, PageTable};

        const KERNEL_STACK_SLOT_SIZE: usize = hal::memory::mebibytes(1);

        let slot_bottom = self.alloc_kernel(KERNEL_STACK_SLOT_SIZE).unwrap();
        let top = slot_bottom + KERNEL_STACK_SLOT_SIZE - 1;
        let stack_bottom = top - initial_size + 1;

        let physical_start = crate::PMM.get().alloc(initial_size / Size4KiB::SIZE);
        self.kernel_page_table
            .lock()
            .map_area(
                stack_bottom,
                physical_start,
                initial_size,
                Flags { writable: true, ..Default::default() },
                crate::PMM.get(),
            )
            .unwrap();

        Some(Stack { top, slot_bottom, stack_bottom, physical_start })
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
