pub mod mailbox;
pub mod result;
pub mod system_object;

pub use system_object::request_system_object;

cfg_if::cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        pub mod raw_x86_64;
        pub use raw_x86_64 as raw;
    } else {
        compile_error!("libpebble does not support this target architecture!");
    }
}

use crate::KernelObjectId;
use result::result_from_syscall_repr;

pub const SYSCALL_YIELD: usize = 0;
pub const SYSCALL_EARLY_LOG: usize = 1;
pub const SYSCALL_REQUEST_SYSTEM_OBJECT: usize = 2;
pub const SYSCALL_MY_ADDRESS_SPACE: usize = 3;
pub const SYSCALL_MAP_MEMORY_OBJECT: usize = 4;
pub const SYSCALL_CREATE_MAILBOX: usize = 5;

pub fn yield_to_kernel() {
    unsafe {
        raw::syscall0(SYSCALL_YIELD);
    }
}

pub fn early_log(message: &str) -> Result<(), ()> {
    match unsafe { raw::syscall2(SYSCALL_EARLY_LOG, message.len(), message as *const str as *const u8 as usize) } {
        0 => Ok(()),
        _ => Err(()),
    }
}

pub fn my_address_space() -> KernelObjectId {
    KernelObjectId::from_syscall_repr(unsafe { raw::syscall0(SYSCALL_MY_ADDRESS_SPACE) })
}

#[derive(Clone, Copy, Debug)]
pub enum MapMemoryObjectError {
    /// The MemoryObject could not be mapped, because there is already a MemoryObject mapped into
    /// the required range of virtual addresses.
    AddressRangeNotFree,
    AccessDeniedToMemoryObject,
    AccessDeniedToAddressSpace,
    NotAMemoryObject,
    NotAnAddressSpace,
}

pub fn map_memory_object(
    memory_object: KernelObjectId,
    address_space: KernelObjectId,
) -> Result<(), MapMemoryObjectError> {
    match unsafe {
        raw::syscall2(SYSCALL_MAP_MEMORY_OBJECT, memory_object.to_syscall_repr(), address_space.to_syscall_repr())
    } {
        0 => Ok(()),
        1 => Err(MapMemoryObjectError::AddressRangeNotFree),
        2 => Err(MapMemoryObjectError::AccessDeniedToMemoryObject),
        3 => Err(MapMemoryObjectError::AccessDeniedToAddressSpace),
        4 => Err(MapMemoryObjectError::NotAMemoryObject),
        5 => Err(MapMemoryObjectError::NotAnAddressSpace),
        _ => panic!("Syscall map_memory_object returned unexpected status code"),
    }
}
