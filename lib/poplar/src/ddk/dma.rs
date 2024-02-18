use crate::memory_object::MappedMemoryObject;
use alloc::sync::Arc;
use core::{
    alloc::{Allocator, Layout},
    mem,
    ptr::{self, NonNull},
    sync::atomic::{AtomicBool, Ordering},
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
            token: AtomicBool::new(false),
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
            token: AtomicBool::new(false),
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
            token: AtomicBool::new(false),
        })
    }
}

pub struct DmaObject<T> {
    pub ptr: NonNull<T>,
    pub phys: usize,
    allocator: Arc<LockedHeap>,
    token: AtomicBool,
}

impl<T> DmaObject<T> {
    pub fn token(&mut self) -> Result<DmaToken, ()> {
        if let Ok(_) = self.token.compare_exchange(false, true, Ordering::Acquire, Ordering::Acquire) {
            Ok(DmaToken {
                ptr: self.ptr.cast(),
                length: mem::size_of::<T>(),
                phys: self.phys,
                token: unsafe { NonNull::new_unchecked(&mut self.token as *mut AtomicBool) },
            })
        } else {
            return Err(());
        }
    }

    pub fn read(&self) -> &T {
        assert!(!self.token_held(), "DmaObject accessed while token held!");
        unsafe { self.ptr.as_ref() }
    }

    pub fn write(&mut self) -> &mut T {
        assert!(!self.token_held(), "DmaObject accessed while token held!");
        unsafe { self.ptr.as_mut() }
    }

    fn token_held(&self) -> bool {
        self.token.load(Ordering::Acquire)
    }
}

impl<T> Drop for DmaObject<T> {
    fn drop(&mut self) {
        assert!(!self.token_held(), "DmaObject dropped while token held!");
        unsafe { self.allocator.deallocate(self.ptr.cast(), Layout::new::<T>()) }
    }
}

pub struct DmaArray<T> {
    pub ptr: NonNull<T>,
    pub length: usize,
    pub phys: usize,
    allocator: Arc<LockedHeap>,
    token: AtomicBool,
}

impl<T> DmaArray<T> {
    pub fn token(&mut self) -> Result<DmaToken, ()> {
        if let Ok(_) = self.token.compare_exchange(false, true, Ordering::Acquire, Ordering::Acquire) {
            Ok(DmaToken {
                ptr: self.ptr.cast(),
                length: self.length,
                phys: self.phys,
                token: unsafe { NonNull::new_unchecked(&mut self.token as *mut AtomicBool) },
            })
        } else {
            return Err(());
        }
    }

    pub fn write(&mut self, index: usize, value: T) {
        assert!(!self.token_held(), "DmaArray accessed while token held!");
        assert!(index < self.length);
        unsafe {
            ptr::write(self.ptr.as_ptr().add(index), value);
        }
    }

    pub fn read(&self, index: usize) -> &T {
        assert!(!self.token_held(), "DmaArray accessed while token held!");
        assert!(index < self.length);
        unsafe { &*self.ptr.as_ptr().add(index) }
    }

    pub fn phys_of_element(&self, index: usize) -> usize {
        self.phys + index * mem::size_of::<T>()
    }

    fn token_held(&self) -> bool {
        self.token.load(Ordering::Acquire)
    }
}

impl<T> Drop for DmaArray<T> {
    fn drop(&mut self) {
        assert!(!self.token_held(), "DmaArray dropped while token held!");
        unsafe { self.allocator.deallocate(self.ptr.cast(), Layout::array::<T>(self.length).unwrap()) }
    }
}

pub struct DmaBuffer {
    pub ptr: NonNull<[u8]>,
    pub length: usize,
    pub phys: usize,
    allocator: Arc<LockedHeap>,
    token: AtomicBool,
}

impl DmaBuffer {
    pub fn token(&mut self) -> Result<DmaToken, ()> {
        if let Ok(_) = self.token.compare_exchange(false, true, Ordering::Acquire, Ordering::Acquire) {
            Ok(DmaToken {
                ptr: self.ptr.cast(),
                length: self.length,
                phys: self.phys,
                token: unsafe { NonNull::new_unchecked(&mut self.token as *mut AtomicBool) },
            })
        } else {
            return Err(());
        }
    }

    pub fn read(&self) -> &[u8] {
        assert!(!self.token_held(), "DmaBuffer accessed while token held!");
        unsafe { self.ptr.as_ref() }
    }

    pub fn write(&mut self) -> &mut [u8] {
        assert!(!self.token_held(), "DmaBuffer accessed while token held!");
        unsafe { self.ptr.as_mut() }
    }

    pub unsafe fn at<T>(&self, offset: usize) -> &T {
        assert!(!self.token_held(), "DmaBuffer accessed while token held!");
        assert!((offset + mem::size_of::<T>()) <= self.length);
        unsafe { &*(self.ptr.byte_add(offset).cast::<T>().as_ptr()) }
    }

    fn token_held(&self) -> bool {
        self.token.load(Ordering::Acquire)
    }
}

impl Drop for DmaBuffer {
    fn drop(&mut self) {
        assert!(!self.token_held(), "DmaBuffer dropped while token held!");
        unsafe { self.allocator.deallocate(self.ptr.cast(), Layout::array::<u8>(self.length).unwrap()) }
    }
}

/// A `DmaToken` refers to an underlying `DmaObject`, `DmaArray`, or `DmaBuffer` while waiting for
/// a DMA transaction to complete. It allows a DMA type to be 'locked' while hardware is accessing
/// it (preventing accesses through the DMA type and erroring if it is dropped while the token is
/// held), and so helps to enforce correct use of DMA memory.
///
/// It also erases the overlying DMA type, and so allows users to operate on any form of DMAable memory.
pub struct DmaToken {
    pub ptr: NonNull<u8>,
    pub length: usize,
    pub phys: usize,
    token: NonNull<AtomicBool>,
}

impl Drop for DmaToken {
    fn drop(&mut self) {
        unsafe {
            self.token.as_ref().store(false, Ordering::Release);
        }
    }
}
