use crate::Platform;

use super::{Pmm, SlabAllocator};
use hal::memory::{FrameSize, PAddr, Size4KiB, VAddr};
use spinning_top::Spinlock;

/*
 * TODO
 *
 * I want to entirely reimagine how we handle virtual memory here. Far more stuff should be in the common kernel rather than the arch-layer, and
 * more stuff should be dynamic. This is required now to handle HPET mapping more correctly, rather than abusing the direct mapping, but has been
 * a weak point of the kernel for a while.
 *
 * Big task: decide whether the VMM should be parameterised by `P` (and therefore can manage the kernel page table itself), in which case the instance
 * needs to be managed by the arch-layer. This should be possible, but might be optimal idk.
 *
 *    - Pass boot info size / info about end of kernel from Seed to kernel
 *    - Maybe split memory module out into top-level PMM and VMM modules?
 *    - Have a concept of the HHDM in the common kernel and retire `Platform::write_to_phys_memory` (this shouldn't really exist at all...)
 *    - Have a concept of an owned portion of the kernel address space that is bound to normal lifetime etc. for mapping stuff dynamically
 *    - Dynamically map the HPET register space if required
 *    - Construct initial layout with HHDM, kernel image, boot info, log ring (?in future)
 *    - Make kernel stacks dynamically-allocated to free up virtual space + remove hard task limit
 */

pub struct Vmm {
    kernel_stack_slots: Spinlock<SlabAllocator>,
    kernel_stack_slot_size: usize,
}

impl Vmm {
    pub fn new(kernel_stacks_bottom: VAddr, kernel_stacks_top: VAddr, kernel_stack_slot_size: usize) -> Vmm {
        Vmm {
            kernel_stack_slots: Spinlock::new(SlabAllocator::new(
                kernel_stacks_bottom,
                kernel_stacks_top,
                kernel_stack_slot_size,
            )),
            kernel_stack_slot_size,
        }
    }

    pub fn alloc_kernel_stack<P>(
        &self,
        initial_size: usize,
        physical_memory_manager: &Pmm,
        kernel_page_table: &mut P::PageTable,
    ) -> Option<Stack>
    where
        P: Platform,
    {
        use hal::memory::{Flags, PageTable};

        let slot_bottom = self.kernel_stack_slots.lock().alloc()?;
        let top = slot_bottom + self.kernel_stack_slot_size - 1;
        let stack_bottom = top - initial_size + 1;

        let physical_start = physical_memory_manager.alloc(initial_size / Size4KiB::SIZE);
        // TODO: bring "master" kernel page tables into this struct?
        kernel_page_table
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
