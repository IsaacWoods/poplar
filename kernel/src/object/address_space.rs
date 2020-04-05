use super::{alloc_kernel_object_id, memory_object::MemoryObject, task::TaskStack, KernelObject, KernelObjectId};
use crate::{memory::PhysicalMemoryManager, per_cpu::KernelPerCpu, slab_allocator::SlabAllocator};
use alloc::{sync::Arc, vec::Vec};
use hal::{
    memory::{FrameAllocator, PageTable, VirtualAddress, MEBIBYTES_TO_BYTES},
    Hal,
};
use libpebble::syscall::MemoryObjectError;
use spin::Mutex;

// TODO: we need some way of getting this from the platform I guess?
// TODO: we've basically made these up
const USER_STACK_BOTTOM: VirtualAddress = VirtualAddress::new(0x00000002_00000000);
const USER_STACK_TOP: VirtualAddress = VirtualAddress::new(0x00000003_ffffffff);
const USER_STACK_SLOT_SIZE: usize = 2 * MEBIBYTES_TO_BYTES;

#[derive(PartialEq, Eq, Debug)]
pub enum State {
    NotActive,
    Active,
}

pub struct AddressSpace<H>
where
    H: Hal<KernelPerCpu>,
{
    pub id: KernelObjectId,
    pub owner: KernelObjectId,
    pub state: Mutex<State>,
    pub memory_objects: Mutex<Vec<Arc<MemoryObject>>>,
    page_table: Mutex<H::PageTable>,
    user_stack_allocator: Mutex<SlabAllocator>,
}

impl<H> AddressSpace<H>
where
    H: Hal<KernelPerCpu>,
{
    pub fn new<A>(owner: KernelObjectId, kernel_page_table: &H::PageTable, allocator: &A) -> Arc<AddressSpace<H>>
    where
        A: FrameAllocator<H::PageTableSize>,
    {
        Arc::new(AddressSpace {
            id: alloc_kernel_object_id(),
            owner,
            state: Mutex::new(State::NotActive),
            memory_objects: Mutex::new(vec![]),
            page_table: Mutex::new(H::PageTable::new_for_address_space(kernel_page_table, allocator)),
            user_stack_allocator: Mutex::new(SlabAllocator::new(
                USER_STACK_BOTTOM,
                USER_STACK_TOP,
                USER_STACK_SLOT_SIZE,
            )),
        })
    }

    pub fn map_memory_object<A>(
        &self,
        memory_object: Arc<MemoryObject>,
        allocator: &A,
    ) -> Result<(), MemoryObjectError>
    where
        A: FrameAllocator<H::PageTableSize>,
    {
        use hal::memory::PagingError;

        self.page_table
            .lock()
            .map_area(
                memory_object.virtual_address,
                memory_object.physical_address,
                memory_object.size,
                memory_object.flags,
                allocator,
            )
            .map_err(|err| match err {
                // XXX: these are explicity enumerated to avoid a bug if variants are added to `PagingError`.
                PagingError::AlreadyMapped => MemoryObjectError::AddressRangeNotFree,
            })?;
        self.memory_objects.lock().push(memory_object);
        Ok(())
    }

    /// Try to allocate a slot for a user stack, and map `initial_size` bytes of it. Returns `None` if no more user
    /// stacks can be allocated in this address space.
    pub fn alloc_user_stack(
        &self,
        initial_size: usize,
        allocator: &PhysicalMemoryManager<H>,
    ) -> Option<TaskStack> {
        use hal::memory::Flags;

        let slot_bottom = self.user_stack_allocator.lock().alloc()?;
        let top = slot_bottom + USER_STACK_SLOT_SIZE - 1;
        let stack_bottom = top - initial_size + 1;

        let physical_start = allocator.alloc_bytes(initial_size);
        self.page_table
            .lock()
            .map_area(
                stack_bottom,
                physical_start,
                initial_size,
                Flags { writable: true, user_accessible: true, ..Default::default() },
                allocator,
            )
            .unwrap();

        Some(TaskStack { top, slot_bottom, stack_bottom })
    }

    pub fn switch_to(&self) {
        assert_eq!(*self.state.lock(), State::NotActive);
        self.page_table.lock().switch_to();
        *self.state.lock() = State::Active;
    }

    pub fn switch_from(&self) {
        assert_eq!(*self.state.lock(), State::Active);
        *self.state.lock() = State::NotActive;
    }
}

impl<H> KernelObject for AddressSpace<H>
where
    H: Hal<KernelPerCpu>,
{
    fn id(&self) -> KernelObjectId {
        self.id
    }
}
