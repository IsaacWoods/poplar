use super::{alloc_kernel_object_id, memory_object::MemoryObject, KernelObject, KernelObjectId};
use alloc::{sync::Arc, vec::Vec};
use hal::{memory::PageTable, Hal};
use libpebble::syscall::MemoryObjectError;
use spin::Mutex;

#[derive(PartialEq, Eq, Debug)]
pub enum State {
    NotActive,
    Active,
}

pub struct AddressSpace<H>
where
    H: Hal,
{
    pub id: KernelObjectId,
    pub owner: KernelObjectId,
    pub state: Mutex<State>,
    pub memory_objects: Mutex<Vec<Arc<MemoryObject>>>,
    page_table: Mutex<H::PageTable>,
}

impl<H> AddressSpace<H>
where
    H: Hal,
{
    pub fn new(
        owner: KernelObjectId,
        kernel_page_table: &H::PageTable,
        allocator: &H::TableAllocator,
    ) -> AddressSpace<H> {
        AddressSpace {
            id: alloc_kernel_object_id(),
            owner,
            state: Mutex::new(State::NotActive),
            memory_objects: Mutex::new(vec![]),
            page_table: Mutex::new(H::PageTable::new_for_address_space(kernel_page_table, allocator)),
        }
    }

    pub fn map_memory_object(
        &self,
        memory_object: Arc<MemoryObject>,
        allocator: &H::TableAllocator,
    ) -> Result<(), MemoryObjectError> {
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
    H: Hal,
{
    fn id(&self) -> KernelObjectId {
        self.id
    }
}
