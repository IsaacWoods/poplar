cfg_if::cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        pub mod raw_x86_64;
        pub use raw_x86_64 as raw;
    } else {
        compile_error!("libpebble does not support this target architecture!");
    }
}

use crate::KernelObjectId;
use bit_field::BitField;

pub const SYSCALL_YIELD: usize = 0;
pub const SYSCALL_EARLY_LOG: usize = 1;
pub const SYSCALL_REQUEST_SYSTEM_OBJECT: usize = 2;
pub const SYSCALL_MY_ADDRESS_SPACE: usize = 3;

pub fn yield_to_kernel() {
    unsafe {
        raw::syscall0(SYSCALL_YIELD);
    }
}

pub fn early_log(message: &str) -> Result<(), ()> {
    match unsafe {
        raw::syscall2(SYSCALL_EARLY_LOG, message.len(), message as *const str as *const u8 as usize)
    } {
        0 => Ok(()),
        _ => Err(()),
    }
}

pub fn my_address_space() -> KernelObjectId {
    KernelObjectId::from_syscall_repr(unsafe { raw::syscall0(SYSCALL_MY_ADDRESS_SPACE) })
}

#[derive(Clone, Copy, Debug)]
pub enum SystemObjectId {
    BackupFramebuffer = 0,
}

#[derive(Clone, Copy, Debug)]
pub enum RequestSystemObjectError {
    /// The requested object ID does point to a valid system object, but the kernel has not created
    /// a corresponding object for it.
    ObjectDoesNotExist,
    /// The requested object ID does not correspond to a valid system object.
    NotAValidId,
    /// The requested object ID is valid, but the requesting task does not have the correct
    /// capabilities to access it.
    PermissionDenied,
}

pub fn request_system_object(id: SystemObjectId) -> Result<KernelObjectId, RequestSystemObjectError> {
    let result = match id {
        /*
         * System objects that don't take any further parameters.
         */
        SystemObjectId::BackupFramebuffer => unsafe {
            raw::syscall1(SYSCALL_REQUEST_SYSTEM_OBJECT, id as usize)
        },
    };

    match result.get_bits(32..64) {
        0 => Ok(KernelObjectId::from_syscall_repr(result)),

        1 => Err(RequestSystemObjectError::ObjectDoesNotExist),
        2 => Err(RequestSystemObjectError::NotAValidId),
        3 => Err(RequestSystemObjectError::PermissionDenied),

        _ => panic!("Syscall request_system_object returned unexpected status code"),
    }
}
