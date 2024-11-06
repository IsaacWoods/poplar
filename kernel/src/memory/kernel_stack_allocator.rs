use super::{PhysicalMemoryManager, SlabAllocator, Stack};
use crate::Platform;
use hal::memory::VAddr;
use spinning_top::Spinlock;

pub struct KernelStackAllocator {
    kernel_stack_slots: Spinlock<SlabAllocator>,
    slot_size: usize,
}

impl KernelStackAllocator {
    pub fn new(stacks_bottom: VAddr, stacks_top: VAddr, slot_size: usize) -> KernelStackAllocator {
        KernelStackAllocator {
            kernel_stack_slots: Spinlock::new(SlabAllocator::new(stacks_bottom, stacks_top, slot_size)),
            slot_size,
        }
    }

    pub fn alloc_kernel_stack<P>(
        &self,
        initial_size: usize,
        physical_memory_manager: &PhysicalMemoryManager,
        kernel_page_table: &mut P::PageTable,
    ) -> Option<Stack>
    where
        P: Platform,
    {
        use hal::memory::{Flags, PageTable};

        let slot_bottom = self.kernel_stack_slots.lock().alloc()?;
        let top = slot_bottom + self.slot_size - 1;
        let stack_bottom = top - initial_size + 1;

        let physical_start = physical_memory_manager.alloc_bytes(initial_size);
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
