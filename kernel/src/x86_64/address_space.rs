use super::{memory::userspace_map, Arch, ARCH};
use crate::object::WrappedKernelObject;
use alloc::vec::Vec;
use hal::memory::{Flags, FrameAllocator, Mapper, Page, Size4KiB, VirtualAddress};
use hal_x86_64::{kernel_map, paging::PageTable};
use libpebble::syscall::MemoryObjectError;
use pebble_util::bitmap::Bitmap;

#[derive(PartialEq, Eq, Debug)]
enum State {
    NotActive,
    Active,
}

pub struct AddressSpace {
    pub table: PageTable,
    state: State,
    memory_objects: Vec<WrappedKernelObject<Arch>>,

    /// We allocate 64 'slots' for task usermode stacks (each of which is 2MiB in size). The task is free to use
    /// less of this space, but can only grow their stack to 2MiB.
    task_user_stack_bitmap: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct TaskUserStack {
    pub top: VirtualAddress,
    pub slot_bottom: VirtualAddress,
    /// Initially, only a portion of the slot will be mapped with actual memory (to decrease memory usage). This
    /// is the actual bottom of the stack until it's grown.
    pub stack_bottom: VirtualAddress,
}

impl AddressSpace {
    pub fn new(arch: &Arch) -> AddressSpace {
        let frame = arch.physical_memory_manager.allocate();
        let mut table = PageTable::new(frame, kernel_map::PHYSICAL_MAPPING_BASE);

        /*
         * Install a copy of the kernel's P3 in each address space. This means the kernel is
         * always mapped, so system calls and interrupts don't page-fault. It's always
         * safe to unwrap the kernel address - if it wasn't there, we wouldn't be able to
         * fetch these instructions.
         */
        let kernel_p3_address =
            arch.kernel_page_table.lock().mapper().p4[kernel_map::KERNEL_P4_ENTRY].address().unwrap();
        table.mapper().p4[kernel_map::KERNEL_P4_ENTRY]
            .set(kernel_p3_address, hal_x86_64::paging::EntryFlags::WRITABLE);

        AddressSpace { table, state: State::NotActive, memory_objects: Vec::new(), task_user_stack_bitmap: 0 }
    }

    pub fn map_memory_object(
        &mut self,
        memory_object: WrappedKernelObject<Arch>,
    ) -> Result<(), MemoryObjectError> {
        {
            use hal::memory::MapperError;
            let mut mapper = self.table.mapper();
            let memory_obj_info = memory_object.object.memory_object().expect("Not a MemoryObject").read();

            mapper
                .map_area(
                    memory_obj_info.virtual_address,
                    memory_obj_info.physical_address,
                    memory_obj_info.size,
                    memory_obj_info.flags,
                    &ARCH.get().physical_memory_manager,
                )
                .map_err(|err| match err {
                    // XXX: this is explicitely enumerated to avoid an error if more errors are added in the future
                    MapperError::AlreadyMapped => MemoryObjectError::AddressRangeNotFree,
                })?;
        }

        self.memory_objects.push(memory_object);
        Ok(())
    }

    pub fn switch_to(&mut self) {
        assert_eq!(self.state, State::NotActive, "Tried to switch to already-active address space!");
        self.table.switch_to();
        self.state = State::Active;
    }

    /// Tell the address space that we are switching to another address space, so it is no longer
    /// the active address space
    // TODO: do we even want to track this state?
    pub fn switch_away_from(&mut self) {
        assert_eq!(self.state, State::Active);
        self.state = State::NotActive;
    }

    pub fn add_stack<A>(&mut self, allocator: &A) -> Option<TaskUserStack>
    where
        A: FrameAllocator<Size4KiB>,
    {
        let index = self.task_user_stack_bitmap.alloc(1)?;

        // TODO: uncouple this from kernel stack slot sizes
        let slot_bottom = userspace_map::USER_STACKS_START + index * kernel_map::STACK_SLOT_SIZE;
        let top = slot_bottom + kernel_map::STACK_SLOT_SIZE - 1;
        let stack_bottom = top - userspace_map::INITIAL_STACK_SIZE;

        for page in Page::contains(stack_bottom)..=Page::contains(top) {
            self.table
                .mapper()
                .map(
                    page,
                    allocator.allocate(),
                    Flags { writable: true, user_accessible: true, ..Default::default() },
                    allocator,
                )
                .unwrap();
        }

        Some(TaskUserStack { top, slot_bottom, stack_bottom })
    }
}
