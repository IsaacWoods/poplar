use super::{alloc_kernel_object_id, memory_object::MemoryObject, KernelObject, KernelObjectId};
use crate::{
    memory::{PhysicalMemoryManager, SlabAllocator, Stack},
    Platform,
};
use alloc::{sync::Arc, vec::Vec};
use hal::memory::{mebibytes, Bytes, FrameAllocator, PageTable, VirtualAddress};
use libpebble::syscall::MapMemoryObjectError;
use spin::Mutex;

// TODO: we need some way of getting this from the platform I guess?
// TODO: we've basically made these up
const USER_STACK_BOTTOM: VirtualAddress = VirtualAddress::new(0x00000002_00000000);
const USER_STACK_TOP: VirtualAddress = VirtualAddress::new(0x00000003_ffffffff);
const USER_STACK_SLOT_SIZE: Bytes = mebibytes(2);

#[derive(PartialEq, Eq, Debug)]
pub enum State {
    NotActive,
    Active,
}

pub struct AddressSpace<P>
where
    P: Platform,
{
    pub id: KernelObjectId,
    pub owner: KernelObjectId,
    pub state: Mutex<State>,
    pub memory_objects: Mutex<Vec<Arc<MemoryObject>>>,
    page_table: Mutex<P::PageTable>,
    user_stack_allocator: Mutex<SlabAllocator>,
}

impl<P> AddressSpace<P>
where
    P: Platform,
{
    pub fn new<A>(owner: KernelObjectId, kernel_page_table: &P::PageTable, allocator: &A) -> Arc<AddressSpace<P>>
    where
        A: FrameAllocator<P::PageTableSize>,
    {
        Arc::new(AddressSpace {
            id: alloc_kernel_object_id(),
            owner,
            state: Mutex::new(State::NotActive),
            memory_objects: Mutex::new(vec![]),
            page_table: Mutex::new(P::PageTable::new_with_kernel_mapped(kernel_page_table, allocator)),
            user_stack_allocator: Mutex::new(SlabAllocator::new(
                USER_STACK_BOTTOM,
                USER_STACK_TOP,
                USER_STACK_SLOT_SIZE,
            )),
        })
    }

    pub fn map_memory_object(
        &self,
        memory_object: Arc<MemoryObject>,
        allocator: &PhysicalMemoryManager,
    ) -> Result<(), MapMemoryObjectError> {
        use hal::memory::PagingError;

        // TODO: handle when the memory object doesn't have a set virtual address (probs take an
        // Option<VirtualAddress> as a param)
        self.page_table
            .lock()
            .map_area(
                memory_object.virtual_address.unwrap(),
                memory_object.physical_address,
                memory_object.size,
                memory_object.flags,
                allocator,
            )
            .map_err(|err| match err {
                // XXX: these are explicity enumerated to avoid a bug if variants are added to `PagingError`.
                PagingError::AlreadyMapped => MapMemoryObjectError::RegionAlreadyMapped,
            })?;
        self.memory_objects.lock().push(memory_object);
        Ok(())
    }

    /// Try to allocate a slot for a user stack, and map `initial_size` bytes of it. Returns `None` if no more user
    /// stacks can be allocated in this address space.
    pub fn alloc_user_stack(&self, initial_size: usize, allocator: &PhysicalMemoryManager) -> Option<Stack> {
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

        Some(Stack { top, slot_bottom, stack_bottom })
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

impl<P> KernelObject for AddressSpace<P>
where
    P: Platform,
{
    fn id(&self) -> KernelObjectId {
        self.id
    }
}
