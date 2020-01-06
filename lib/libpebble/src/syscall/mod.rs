pub mod mailbox;
pub mod result;
pub mod system_object;

pub use mailbox::{create_mailbox, wait_for_mail};
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
use result::{define_error_type, result_from_syscall_repr, status_from_syscall_repr};

pub const SYSCALL_YIELD: usize = 0;
pub const SYSCALL_EARLY_LOG: usize = 1;
pub const SYSCALL_REQUEST_SYSTEM_OBJECT: usize = 2;
pub const SYSCALL_MY_ADDRESS_SPACE: usize = 3;
pub const SYSCALL_MAP_MEMORY_OBJECT: usize = 4;
pub const SYSCALL_CREATE_MAILBOX: usize = 5;
pub const SYSCALL_WAIT_FOR_MAIL: usize = 6;

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

define_error_type!(MemoryObjectMappingError {
    AddressRangeNotFree => 1,
    AccessDeniedToMemoryObject => 2,
    AccessDeniedToAddressSpace => 3,
    NotAMemoryObject => 4,
    NotAnAddressSpace => 5,
});

pub fn map_memory_object(
    memory_object: KernelObjectId,
    address_space: KernelObjectId,
) -> Result<(), MemoryObjectMappingError> {
    status_from_syscall_repr(unsafe {
        raw::syscall2(SYSCALL_MAP_MEMORY_OBJECT, memory_object.to_syscall_repr(), address_space.to_syscall_repr())
    })
}
