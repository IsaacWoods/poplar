use crate::memory_object::MappedMemoryObject;
use alloc::sync::Arc;
use core::{
    alloc::{Allocator, Layout},
    mem,
    ptr::{self, NonNull},
};
use linked_list_allocator::LockedHeap;

pub struct DmaPool {
    memory: MappedMemoryObject,
    allocator: Arc<LockedHeap>,
}

impl DmaPool {
    pub fn new(memory: MappedMemoryObject) -> DmaPool {
        let allocator = Arc::new(unsafe { LockedHeap::new(memory.ptr() as *mut u8, memory.inner.size) });
        DmaPool { memory, allocator }
    }

    pub fn alloc<T>(&self) -> Result<DmaObject<T>, ()> {
        let ptr = self.allocator.allocate(Layout::new::<T>()).map_err(|_| ())?.cast::<T>();
        Ok(DmaObject {
            ptr,
            phys: self.memory.map_address(ptr.as_ptr() as usize).unwrap(),
            allocator: self.allocator.clone(),
        })
    }

    pub fn alloc_array<T>(&self, length: usize) -> Result<DmaArray<T>, ()> {
        let ptr = self.allocator.allocate(Layout::array::<T>(length).unwrap()).map_err(|_| ())?.cast::<T>();
        Ok(DmaArray {
            ptr,
            length,
            phys: self.memory.map_address(ptr.as_ptr() as usize).unwrap(),
            allocator: self.allocator.clone(),
        })
    }
}

pub struct DmaObject<T> {
    pub ptr: NonNull<T>,
    pub phys: usize,
    allocator: Arc<LockedHeap>,
}

impl<T> DmaObject<T> {
    pub fn write(&mut self, value: T) {
        unsafe {
            ptr::write(self.ptr.as_ptr(), value);
        }
    }
}

impl<T> Drop for DmaObject<T> {
    fn drop(&mut self) {
        unsafe { self.allocator.deallocate(self.ptr.cast(), Layout::new::<T>()) }
    }
}

pub struct DmaArray<T> {
    pub ptr: NonNull<T>,
    pub length: usize,
    pub phys: usize,
    allocator: Arc<LockedHeap>,
}

impl<T> DmaArray<T> {
    pub fn write(&mut self, index: usize, value: T) {
        assert!(index < self.length);
        unsafe {
            ptr::write(self.ptr.as_ptr().add(index), value);
        }
    }

    pub fn phys_of_element(&self, index: usize) -> usize {
        self.phys + index * mem::size_of::<T>()
    }
}

impl<T> Drop for DmaArray<T> {
    fn drop(&mut self) {
        unsafe { self.allocator.deallocate(self.ptr.cast(), Layout::array::<T>(self.length).unwrap()) }
    }
}
