use crate::memory_object::MappedMemoryObject;
use alloc::sync::Arc;
use core::{
    alloc::{Allocator, Layout},
    mem,
    ops::{Deref, DerefMut},
    ptr::{self, NonNull},
};
use linked_list_allocator::LockedHeap;

pub struct DmaPool {
    memory: MappedMemoryObject,
    // TODO: in the future, something like a slab allocator would probably be better (no need to
    // waste space inside the physically-mapped region, and likely to map many of the same size)
    allocator: Arc<LockedHeap>,
}

impl DmaPool {
    pub fn new(memory: MappedMemoryObject) -> DmaPool {
        let allocator = Arc::new(unsafe { LockedHeap::new(memory.ptr() as *mut u8, memory.inner.size) });
        DmaPool { memory, allocator }
    }

    pub fn create<T>(&self, value: T) -> Result<DmaObject<T>, ()> {
        let ptr = self.allocator.allocate(Layout::new::<T>()).map_err(|_| ())?.cast::<T>();
        unsafe {
            ptr::write(ptr.as_ptr(), value);
        }
        Ok(DmaObject {
            ptr,
            phys: self.memory.virt_to_phys(ptr.as_ptr() as usize).unwrap(),
            allocator: self.allocator.clone(),
        })
    }

    pub fn create_array<T>(&self, length: usize, value: T) -> Result<DmaArray<T>, ()>
    where
        T: Copy,
    {
        let ptr = self.allocator.allocate(Layout::array::<T>(length).unwrap()).map_err(|_| ())?.cast::<T>();
        for i in 0..length {
            unsafe {
                ptr::write(ptr.as_ptr().add(i), value);
            }
        }
        Ok(DmaArray {
            ptr,
            length,
            phys: self.memory.virt_to_phys(ptr.as_ptr() as usize).unwrap(),
            allocator: self.allocator.clone(),
        })
    }

    pub fn create_buffer(&self, length: usize) -> Result<DmaBuffer, ()> {
        let ptr: NonNull<[u8]> = self.allocator.allocate(Layout::array::<u8>(length).unwrap()).map_err(|_| ())?;
        let slice = unsafe { ptr.as_uninit_slice_mut() };
        for i in 0..length {
            slice[i].write(0x00);
        }

        Ok(DmaBuffer {
            ptr,
            length,
            phys: self.memory.virt_to_phys(ptr.cast::<u8>().as_ptr() as usize).unwrap(),
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

impl<T> Deref for DmaObject<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr.as_ptr() }
    }
}

impl<T> DerefMut for DmaObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.ptr.as_ptr() }
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

    pub fn read(&self, index: usize) -> &T {
        assert!(index < self.length);
        unsafe { &*self.ptr.as_ptr().add(index) }
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

pub struct DmaBuffer {
    pub ptr: NonNull<[u8]>,
    pub length: usize,
    pub phys: usize,
    allocator: Arc<LockedHeap>,
}

impl Deref for DmaBuffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr.as_ptr() }
    }
}

impl DerefMut for DmaBuffer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.ptr.as_ptr() }
    }
}

impl Drop for DmaBuffer {
    fn drop(&mut self) {
        unsafe { self.allocator.deallocate(self.ptr.cast(), Layout::array::<u8>(self.length).unwrap()) }
    }
}
