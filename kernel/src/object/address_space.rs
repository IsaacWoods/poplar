use super::{alloc_kernel_object_id, memory_object::MemoryObject, KernelObject, KernelObjectId, KernelObjectType};
use crate::{
    pmm::Pmm,
    vmm::{Stack, Vmm},
    Platform,
};
use alloc::{collections::btree_map::BTreeMap, sync::Arc};
use hal::memory::{mebibytes, Bytes, FrameAllocator, FrameSize, PageTable, Size4KiB, VAddr};
use mulch::bitmap::Bitmap;
use poplar::syscall::MapMemoryObjectError;
use spinning_top::Spinlock;

// TODO: we need some way of getting this from the platform I guess?
// TODO: we've basically made these up
const USER_STACK_BASE: VAddr = VAddr::new(0x00000002_00000000);
const USER_STACK_SLOT_SIZE: Bytes = mebibytes(1);

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
    pub mappings: Spinlock<BTreeMap<VAddr, Arc<MemoryObject>>>,
    pub page_table: Spinlock<P::PageTable>,
    slot_bitmap: Spinlock<u64>,
}

impl<P> AddressSpace<P>
where
    P: Platform,
{
    pub fn new(owner: KernelObjectId) -> Arc<AddressSpace<P>> {
        Arc::new(AddressSpace {
            id: alloc_kernel_object_id(),
            owner,
            state: Spinlock::new(State::NotActive),
            mappings: Spinlock::new(BTreeMap::new()),
            page_table: Spinlock::new(P::new_task_page_tables()),
            slot_bitmap: Spinlock::new(0),
        })
    }

    pub fn map_memory_object(
        &self,
        memory_object: Arc<MemoryObject>,
        virtual_address: VAddr,
        allocator: &Pmm,
    ) -> Result<(), MapMemoryObjectError> {
        use hal::memory::PagingError;

        {
            let mut current_virtual = virtual_address;
            let inner = memory_object.inner.lock();
            for (backing, size) in &inner.backing {
                self.page_table
                    .lock()
                    .map_area(current_virtual, *backing, *size, inner.flags, allocator)
                    .map_err(|err| match err {
                        // XXX: these are explicity enumerated to avoid a bug if variants are added to `PagingError`.
                        PagingError::AlreadyMapped => MapMemoryObjectError::RegionAlreadyMapped,
                    })?;
                current_virtual += *size;
            }
        }

        self.mappings.lock().insert(virtual_address, memory_object);
        Ok(())
    }

    /// Try to allocate a slot for a Task. Creates a user stack with `initial_stack_size` bytes initially
    /// allocated. Returs `None` if no more tasks can be created in this Address Space.
    pub fn alloc_task_slot(&self, initial_stack_size: usize, allocator: &Pmm) -> Option<TaskSlot> {
        use hal::memory::Flags;

        let index = self.slot_bitmap.lock().alloc(1)?;

        let user_stack = {
            let slot_bottom = USER_STACK_BASE + USER_STACK_SLOT_SIZE * index;
            let top = slot_bottom + USER_STACK_SLOT_SIZE - 1;
            let stack_bottom = (top + 1) - initial_stack_size;

            let physical_start = allocator.alloc(initial_stack_size / Size4KiB::SIZE);
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

    fn typ(&self) -> KernelObjectType {
        KernelObjectType::AddressSpace
    }
}
