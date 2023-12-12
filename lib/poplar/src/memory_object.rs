use crate::{
    syscall::{self, CreateMemoryObjectError, MapMemoryObjectError, MemoryObjectFlags},
    Handle,
};
use core::ptr;

pub struct MemoryObject {
    pub handle: Handle,
    pub size: usize,
    pub flags: MemoryObjectFlags,
    pub phys_address: Option<usize>,
}

impl MemoryObject {
    pub unsafe fn create(size: usize, flags: MemoryObjectFlags) -> Result<MemoryObject, CreateMemoryObjectError> {
        let handle = unsafe { crate::syscall::create_memory_object(size, flags, ptr::null_mut())? };
        Ok(MemoryObject { handle, size, flags, phys_address: None })
    }

    pub unsafe fn create_physical(
        size: usize,
        flags: MemoryObjectFlags,
    ) -> Result<MemoryObject, CreateMemoryObjectError> {
        let mut phys_address = 0usize;
        let handle =
            unsafe { crate::syscall::create_memory_object(size, flags, &mut phys_address as *mut usize)? };
        Ok(MemoryObject { handle, size, flags, phys_address: Some(phys_address) })
    }

    pub unsafe fn map(self) -> Result<MappedMemoryObject, MapMemoryObjectError> {
        let mut address = 0usize;
        unsafe {
            syscall::map_memory_object(self.handle, Handle::ZERO, None, &mut address as *mut usize)?;
        }
        Ok(MappedMemoryObject { inner: self, mapped_at: address })
    }

    pub unsafe fn map_at(self, address: usize) -> Result<MappedMemoryObject, MapMemoryObjectError> {
        unsafe {
            syscall::map_memory_object(self.handle, Handle::ZERO, Some(address), ptr::null_mut())?;
        }
        Ok(MappedMemoryObject { inner: self, mapped_at: address })
    }
}

pub struct MappedMemoryObject {
    inner: MemoryObject,
    /// The virtual address (address in the task's address space) the object has been mapped at.
    mapped_at: usize,
}

impl MappedMemoryObject {
    pub fn ptr(&self) -> *const u8 {
        self.mapped_at as *const u8
    }

    /// For `MemoryObject`s with a known physical mapping, translate a given physical address into
    /// the corresponding virtual address (the address in the task's address space).
    pub fn map_address(&self, physical: usize) -> Option<usize> {
        self.inner.phys_address.map(|phys_base| physical - phys_base + self.mapped_at)
    }
}
