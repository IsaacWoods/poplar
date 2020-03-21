use super::{alloc_kernel_object_id, memory_object::MemoryObject, KernelObject, KernelObjectId};
use alloc::{sync::Arc, vec::Vec};
use hal::{memory::PageTable, Hal};
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
}

impl<H> KernelObject for AddressSpace<H>
where
    H: Hal,
{
    fn id(&self) -> KernelObjectId {
        self.id
    }
}
