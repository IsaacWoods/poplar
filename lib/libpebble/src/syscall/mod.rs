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

use crate::Handle;
use bit_field::BitField;
use result::{define_error_type, handle_from_syscall_repr, status_from_syscall_repr};

pub const SYSCALL_YIELD: usize = 0;
pub const SYSCALL_EARLY_LOG: usize = 1;
pub const SYSCALL_REQUEST_SYSTEM_OBJECT: usize = 2;
pub const SYSCALL_MY_ADDRESS_SPACE: usize = 3;
pub const SYSCALL_CREATE_MEMORY_OBJECT: usize = 4;
pub const SYSCALL_MAP_MEMORY_OBJECT: usize = 5;
pub const SYSCALL_CREATE_MAILBOX: usize = 6;
pub const SYSCALL_WAIT_FOR_MAIL: usize = 7;

pub fn yield_to_kernel() {
    unsafe {
        raw::syscall0(SYSCALL_YIELD);
    }
}

define_error_type!(EarlyLogError {
    MessageTooLong => 1,
    MessageNotValidUtf8 => 2,
    TaskDoesNotHaveCorrectCapability => 3,
});

pub fn early_log(message: &str) -> Result<(), EarlyLogError> {
    status_from_syscall_repr(unsafe {
        raw::syscall2(SYSCALL_EARLY_LOG, message.len(), message as *const str as *const u8 as usize)
    })
}

pub fn my_address_space() -> Handle {
    Handle(unsafe { raw::syscall0(SYSCALL_MY_ADDRESS_SPACE) } as u16)
}

define_error_type!(MemoryObjectError {
    /*
     * These errors are returned by `create_memory_object`.
     */
    InvalidVirtualAddress => 1,
    InvalidFlags => 2,
    InvalidSize => 3,

    /*
     * These errors are returned by `map_memory_object`.
     */
    AddressRangeNotFree => 4,
    AccessDeniedToMemoryObject => 5,
    AccessDeniedToAddressSpace => 6,
    NotAMemoryObject => 7,
    NotAnAddressSpace => 8,
});

/// Create a MemoryObject kernel object at the given virtual address, with the given size (in bytes). Returns the
/// ID of the new MemoryObject, if the call was successful.
pub fn create_memory_object(
    virtual_address: usize,
    size: usize,
    writable: bool,
    executable: bool,
) -> Result<Handle, MemoryObjectError> {
    let mut flags = 0usize;
    flags.set_bit(0, writable);
    flags.set_bit(1, executable);

    handle_from_syscall_repr(unsafe { raw::syscall3(SYSCALL_CREATE_MEMORY_OBJECT, virtual_address, size, flags) })
}

pub fn map_memory_object(memory_object: Handle, address_space: Handle) -> Result<(), MemoryObjectError> {
    status_from_syscall_repr(unsafe {
        raw::syscall2(SYSCALL_MAP_MEMORY_OBJECT, memory_object.0 as usize, address_space.0 as usize)
    })
}
