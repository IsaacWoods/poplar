pub mod get_framebuffer;
pub mod result;

pub use get_framebuffer::{get_framebuffer, FramebufferInfo, GetFramebufferError, PixelFormat};

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
pub const SYSCALL_GET_FRAMEBUFFER: usize = 2;
pub const SYSCALL_CREATE_MEMORY_OBJECT: usize = 3;
pub const SYSCALL_MAP_MEMORY_OBJECT: usize = 4;

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

define_error_type!(CreateMemoryObjectError {
    InvalidVirtualAddress => 1,
    InvalidFlags => 2,
    InvalidSize => 3,
});

/// Create a MemoryObject kernel object at the given virtual address, with the given size (in bytes). Returns a
/// handle to the new MemoryObject, if the call was successful.
pub fn create_memory_object(
    virtual_address: usize,
    size: usize,
    writable: bool,
    executable: bool,
) -> Result<Handle, CreateMemoryObjectError> {
    let mut flags = 0usize;
    flags.set_bit(0, writable);
    flags.set_bit(1, executable);

    handle_from_syscall_repr(unsafe { raw::syscall3(SYSCALL_CREATE_MEMORY_OBJECT, virtual_address, size, flags) })
}

define_error_type!(MapMemoryObjectError {
    InvalidHandle => 1,
    RegionAlreadyMapped => 2,
    NotAMemoryObject => 3,
    NotAnAddressSpace => 4,
});

pub fn map_memory_object(
    memory_object: Handle,
    address_space: Handle,
    address_pointer: *mut usize,
) -> Result<(), MapMemoryObjectError> {
    status_from_syscall_repr(unsafe {
        raw::syscall3(
            SYSCALL_MAP_MEMORY_OBJECT,
            memory_object.0 as usize,
            address_space.0 as usize,
            address_pointer as usize,
        )
    })
}
