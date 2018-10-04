use super::{BootServices, Pool};
use core::{mem, slice};
use crate::memory::{MemoryDescriptor, MemoryMap, MemoryType, PhysicalAddress};
use crate::types::Status;

/// Type of memory allocation to perform
#[repr(C)]
pub enum AllocateType {
    AllocateAnyPages,
    AllocateMaxAddress,
    AllocateAddress,
    MaxAllocateType,
}

impl BootServices {
    /// Allocates memory pages from the system
    pub fn allocate_pages(
        &self,
        allocation_type: AllocateType,
        memory_type: MemoryType,
        pages: usize,
        memory: &mut PhysicalAddress,
    ) -> Result<(), Status> {
        (self._allocate_pages)(allocation_type, memory_type, pages, memory)
            .as_result()
            .map(|_| ())
    }

    /// Frees memory pages
    pub fn free_pages(&self, memory: PhysicalAddress, pages: usize) -> Result<(), Status> {
        (self._free_pages)(memory, pages).as_result().map(|_| ())
    }

    /// Returns the current memory map
    pub fn get_memory_map(&self) -> Result<MemoryMap, Status> {
        let mut map = MemoryMap {
            buffer: 0 as *mut MemoryDescriptor,
            descriptor_size: 0,
            descriptor_version: 0,
            key: 0,
            size: 0,
        };

        // Make a dummy call to _get_memory_map to get details about descriptor and map size
        let res = (self._get_memory_map)(
            &mut map.size,
            map.buffer,
            &mut map.key,
            &mut map.descriptor_size,
            &mut map.descriptor_version,
        );
        if res != Status::BufferTooSmall {
            return Err(res);
        }

        // Get a suitably-sized buffer with a little breathing room
        map.size += map.descriptor_size * 3;
        map.buffer = self.allocate_pool(MemoryType::LoaderData, map.size)? as *mut MemoryDescriptor;

        // Make the true call to _get_memory_map with a real buffer
        (self._get_memory_map)(
            &mut map.size,
            map.buffer,
            &mut map.key,
            &mut map.descriptor_size,
            &mut map.descriptor_version,
        )
        .as_result()
        .map(|_| map)
    }

    /// Allocates pool memory
    pub fn allocate_pool(&self, pool_type: MemoryType, size: usize) -> Result<*mut u8, Status> {
        let mut buffer: *mut u8 = 0 as *mut u8;
        (self._allocate_pool)(pool_type, size, &mut buffer)
            .as_result()
            .map(|_| buffer)
    }

    /// Returns pool memory to the system
    pub fn free_pool(&self, buffer: *mut u8) -> Result<(), Status> {
        (self._free_pool)(buffer).as_result().map(|_| ())
    }

    /// Allocates a slice from pool memory
    pub fn allocate_slice<T>(&self, count: usize) -> Result<Pool<[T]>, Status> {
        let ptr = self.allocate_pool(MemoryType::LoaderData, count * mem::size_of::<T>())?;
        unsafe {
            Ok(Pool::new_unchecked(slice::from_raw_parts_mut(
                ptr as *mut T,
                count,
            )))
        }
    }

    /// Fills the buffer with the specified value
    ///
    /// # Safety
    ///
    /// This method is inherently unsafe, because it can modify the contents of any specified memory
    /// location. The caller is responsible for
    pub unsafe fn set_mem(&self, buffer: *mut u8, size: usize, value: u8) {
        (self._set_mem)(buffer, size, value);
    }
}
