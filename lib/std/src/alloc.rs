pub use alloc_crate::alloc::*;

use core::ptr::NonNull;
use linked_list_allocator::LockedHeap;
use poplar::{
    memory_object::{MappedMemoryObject, MemoryObject},
    syscall::MemoryObjectFlags,
};
use spinning_top::Spinlock;

/// Virtual address to put the heap at. This could be dynamically chosen in the future.
const HEAP_START: usize = 0x600000000;
/// The size of the heap on the first allocation made
const INITIAL_HEAP_SIZE: usize = 0x4000;

#[global_allocator]
static ALLOCATOR: ManagedHeap = ManagedHeap::empty();

struct ManagedHeap {
    inner: LockedHeap,
    mapped_heap: Spinlock<Option<MappedMemoryObject>>,
}

impl ManagedHeap {
    const fn empty() -> ManagedHeap {
        ManagedHeap { inner: LockedHeap::empty(), mapped_heap: Spinlock::new(None) }
    }
}

unsafe impl GlobalAlloc for ManagedHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let attempted_alloc = self.inner.lock().allocate_first_fit(layout);
        if let Ok(ptr) = attempted_alloc {
            ptr.as_ptr()
        } else {
            /*
             * The allocation failed. Initialize the heap if it hasn't been already, or try to
             * extend it if it has.
             */
            if self.mapped_heap.lock().is_none() {
                let initial_size = usize::min(INITIAL_HEAP_SIZE, layout.size());
                let memory = MemoryObject::create(initial_size, MemoryObjectFlags::WRITABLE).unwrap();
                *self.mapped_heap.lock() = Some(memory.map_at(HEAP_START).unwrap());
                self.inner.lock().init(HEAP_START as *mut u8, initial_size);

                // Recurse to make the allocation so we can extend the heap if needed
                self.alloc(layout)
            } else {
                {
                    let mut memory_object = self.mapped_heap.lock();
                    let current_size = memory_object.as_ref().unwrap().inner.size;
                    let new_size = usize::min(current_size * 2, current_size + layout.size() + 256);
                    memory_object.as_mut().unwrap().resize(new_size).unwrap();
                    self.inner.lock().extend(new_size - current_size);
                }

                // Recurse to make the allocation / extend the heap more
                self.alloc(layout)
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.inner.lock().deallocate(NonNull::new_unchecked(ptr), layout);
    }
}
