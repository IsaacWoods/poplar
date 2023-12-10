use super::{alloc_kernel_object_id, memory_object::MemoryObject, KernelObject, KernelObjectId};
use crate::{
    memory::{PhysicalMemoryManager, Stack},
    Platform,
};
use alloc::{sync::Arc, vec::Vec};
use hal::memory::{mebibytes, Bytes, FrameAllocator, PageTable, VAddr};
use poplar::syscall::MapMemoryObjectError;
use poplar_util::bitmap::Bitmap;
use spinning_top::Spinlock;

const MAX_TASKS: usize = 64;

// TODO: we need some way of getting this from the platform I guess?
// TODO: we've basically made these up
const USER_STACK_BOTTOM: VAddr = VAddr::new(0x00000002_00000000);
const USER_STACK_TOP: VAddr = VAddr::new(0x00000003_ffffffff);
const USER_STACK_SLOT_SIZE: Bytes = mebibytes(4);

#[derive(PartialEq, Eq, Debug)]
pub enum State {
    NotActive,
    Active,
}

#[derive(Debug)]
pub struct TaskSlot {
    pub index: usize,
    pub user_stack: Stack,
}

#[derive(Debug)]
pub struct AddressSpace<P>
where
    P: Platform,
{
    pub id: KernelObjectId,
    pub owner: KernelObjectId,
    pub state: Spinlock<State>,
    pub memory_objects: Spinlock<Vec<Arc<MemoryObject>>>,
    page_table: Spinlock<P::PageTable>,
    slot_bitmap: Spinlock<u64>,
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
            state: Spinlock::new(State::NotActive),
            memory_objects: Spinlock::new(vec![]),
            page_table: Spinlock::new(P::PageTable::new_with_kernel_mapped(kernel_page_table, allocator)),
            slot_bitmap: Spinlock::new(0),
        })
    }

    pub fn map_memory_object(
        &self,
        memory_object: Arc<MemoryObject>,
        virtual_address: Option<VAddr>,
        allocator: &PhysicalMemoryManager,
    ) -> Result<(), MapMemoryObjectError> {
        use hal::memory::PagingError;

        let virtual_address = if virtual_address.is_some() {
            assert!(memory_object.virtual_address.is_none());
            virtual_address.unwrap()
        } else {
            memory_object.virtual_address.unwrap()
        };

        self.page_table
            .lock()
            .map_area(
                virtual_address,
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

    /// Try to allocate a slot for a Task. Creates a user stack with `initial_stack_size` bytes initially
    /// allocated. Returs `None` if no more tasks can be created in this Address Space.
    pub fn alloc_task_slot(
        &self,
        initial_stack_size: usize,
        allocator: &PhysicalMemoryManager,
    ) -> Option<TaskSlot> {
        use hal::memory::Flags;

        let index = self.slot_bitmap.lock().alloc(1)?;

        let user_stack = {
            let slot_bottom = USER_STACK_BOTTOM + USER_STACK_SLOT_SIZE * index;
            let top = slot_bottom + USER_STACK_SLOT_SIZE - 1;
            let stack_bottom = (top + 1) - initial_stack_size;

            let physical_start = allocator.alloc_bytes(initial_stack_size);
            self.page_table
                .lock()
                .map_area(
                    stack_bottom,
                    physical_start,
                    initial_stack_size,
                    Flags { writable: true, user_accessible: true, ..Default::default() },
                    allocator,
                )
                .unwrap();

            Stack { top, slot_bottom, stack_bottom, physical_start }
        };

        Some(TaskSlot { index, user_stack })
    }

    pub fn switch_to(&self) {
        assert_eq!(*self.state.lock(), State::NotActive);
        unsafe {
            self.page_table.lock().switch_to();
        }
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
