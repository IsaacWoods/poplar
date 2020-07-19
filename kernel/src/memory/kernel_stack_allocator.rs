use super::{PhysicalMemoryManager, SlabAllocator, Stack};
use crate::Platform;
use core::marker::PhantomData;
use hal::memory::VirtualAddress;
use spin::Mutex;

pub struct KernelStackAllocator<P>
where
    P: Platform,
{
    kernel_stack_slots: Mutex<SlabAllocator>,
    slot_size: usize,
    _phantom: PhantomData<P>,
}

impl<P> KernelStackAllocator<P>
where
    P: Platform,
{
    pub fn new(
        stacks_bottom: VirtualAddress,
        stacks_top: VirtualAddress,
        slot_size: usize,
    ) -> KernelStackAllocator<P> {
        KernelStackAllocator {
            kernel_stack_slots: Mutex::new(SlabAllocator::new(stacks_bottom, stacks_top, slot_size)),
            slot_size,
            _phantom: PhantomData,
        }
    }

    pub fn alloc_kernel_stack(
        &self,
        initial_size: usize,
        physical_memory_manager: &PhysicalMemoryManager,
        kernel_page_table: &mut P::PageTable,
    ) -> Option<Stack> {
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

        Some(Stack { top, slot_bottom, stack_bottom })
    }
}
