use super::{memory::userspace_map, Arch, ARCH};
use crate::object::WrappedKernelObject;
use alloc::vec::Vec;
use boot_info_x86_64::kernel_map;
use libpebble::syscall::MemoryObjectError;
use pebble_util::bitmap::Bitmap;
use x86_64::memory::{
    EntryFlags,
    Frame,
    FrameAllocator,
    FrameSize,
    Page,
    PageTable,
    Size4KiB,
    TranslationResult,
    VirtualAddress,
};

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
        table.mapper().p4[kernel_map::KERNEL_P4_ENTRY].set(kernel_p3_address, EntryFlags::WRITABLE);

        AddressSpace { table, state: State::NotActive, memory_objects: Vec::new(), task_user_stack_bitmap: 0 }
    }

    pub fn map_memory_object(
        &mut self,
        memory_object: WrappedKernelObject<Arch>,
    ) -> Result<(), MemoryObjectError> {
        {
            let mut mapper = self.table.mapper();
            let memory_obj_info = memory_object.object.memory_object().expect("Not a MemoryObject").read();

            let start_page = Page::<Size4KiB>::starts_with(memory_obj_info.virtual_address);
            let pages = start_page..(start_page + (memory_obj_info.size / Size4KiB::SIZE));

            let start_frame = Frame::<Size4KiB>::starts_with(memory_obj_info.physical_address);
            let frames = start_frame..(start_frame + (memory_obj_info.size / Size4KiB::SIZE));

            /*
             * Check that the entire range of pages we'll be mapping into is currently free.
             */
            for page in pages.clone() {
                match mapper.translate(page.start_address) {
                    TranslationResult::NotMapped => (),
                    _ => return Err(MemoryObjectError::AddressRangeNotFree),
                }
            }

            // TODO: move to map_area_to to use better mapping algorithm
            for (page, frame) in pages.zip(frames) {
                mapper.map_to(page, frame, memory_obj_info.flags, &ARCH.get().physical_memory_manager).unwrap();
            }
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
        A: FrameAllocator,
    {
        let index = self.task_user_stack_bitmap.alloc(1)?;

        // TODO: uncouple this from kernel stack slot sizes
        let slot_bottom = userspace_map::USER_STACKS_START + index * kernel_map::STACK_SLOT_SIZE;
        let top = slot_bottom + kernel_map::STACK_SLOT_SIZE - 1;
        let stack_bottom = top - userspace_map::INITIAL_STACK_SIZE;

        for page in Page::contains(stack_bottom)..=Page::contains(top) {
            self.table.mapper().map(page, EntryFlags::WRITABLE | EntryFlags::USER_ACCESSIBLE, allocator).unwrap();
        }

        Some(TaskUserStack { top, slot_bottom, stack_bottom })
    }
}
