mod slab_allocator;

use crate::{pmm::Pmm, Platform};
use core::sync::atomic::{AtomicUsize, Ordering};
use hal::memory::{Flags, FrameSize, PAddr, PageTable, Size4KiB, VAddr};
use slab_allocator::SlabAllocator;
use spinning_top::Spinlock;

pub struct Vmm<P: Platform> {
    pub kernel_page_table: Spinlock<P::PageTable>,
    // Tracks the next available address in the kernel dynamic allocation map
    // TODO: this should be replaced by some sort of tree
    kernel_next_available: AtomicUsize,
    kernel_end_of_dynamic_area: VAddr,
    kernel_stack_slots: Spinlock<SlabAllocator>,
    kernel_stack_slot_size: usize,
}

impl<P> Vmm<P>
where
    P: Platform,
{
    pub fn new(
        kernel_page_table: P::PageTable,
        kernel_stacks_bottom: VAddr,
        kernel_stacks_top: VAddr,
        kernel_stack_slot_size: usize,
    ) -> Vmm<P> {
        Vmm {
            kernel_page_table: Spinlock::new(kernel_page_table),
            kernel_next_available: AtomicUsize::new(usize::from(P::KERNEL_DYNAMIC_AREA_BASE)),
            // TODO: I guess this could not always be the case? Maybe use another constant?
            kernel_end_of_dynamic_area: P::KERNEL_IMAGE_BASE,
            kernel_stack_slots: Spinlock::new(SlabAllocator::new(
                kernel_stacks_bottom,
                kernel_stacks_top,
                kernel_stack_slot_size,
            )),
            kernel_stack_slot_size,
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
        tracing::info!("Asked to map phys addr {:#x} into kernel dynamic area. VAddr = {:#x}", addr, virt);
        Some(virt)
    }

    // TODO: this could probably just be done into the dynamic memory area??
    pub fn alloc_kernel_stack(&self, initial_size: usize, physical_memory_manager: &Pmm) -> Option<Stack> {
        use hal::memory::{Flags, PageTable};

        let slot_bottom = self.kernel_stack_slots.lock().alloc()?;
        let top = slot_bottom + self.kernel_stack_slot_size - 1;
        let stack_bottom = top - initial_size + 1;

        let physical_start = physical_memory_manager.alloc(initial_size / Size4KiB::SIZE);
        self.kernel_page_table
            .lock()
            .map_area(
                stack_bottom,
                physical_start,
                initial_size,
                Flags { writable: true, ..Default::default() },
                physical_memory_manager,
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
