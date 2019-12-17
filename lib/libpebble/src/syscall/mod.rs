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
pub const SYSCALL_MAP_MEMORY_OBJECT: usize = 4;

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

#[derive(Clone, Copy, Debug)]
pub enum SystemObjectId {
    BackupFramebuffer { info_address: *mut FramebufferSystemObjectInfo },
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
    AccessDenied,
}

/// This is a type representing the information that the kernel will write into the address supplied by userspace
/// when requesting the `BackupFramebuffer` system object.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct FramebufferSystemObjectInfo {
    pub address: usize,

    pub width: u16,
    pub height: u16,
    pub stride: u16,

    /// The representation of pixels in the supplied framebuffer.
    /// 0 = RGB32
    /// 1 = BGR32
    pub pixel_format: u8,
}

pub fn request_system_object(id: SystemObjectId) -> Result<KernelObjectId, RequestSystemObjectError> {
    const BACKUP_FRAMEBUFFER_ID: usize = 0;

    let result = match id {
        SystemObjectId::BackupFramebuffer { info_address } => unsafe {
            raw::syscall2(SYSCALL_REQUEST_SYSTEM_OBJECT, BACKUP_FRAMEBUFFER_ID, info_address as usize)
        },
    };

    match result.get_bits(32..64) {
        0 => Ok(KernelObjectId::from_syscall_repr(result)),

        1 => Err(RequestSystemObjectError::ObjectDoesNotExist),
        2 => Err(RequestSystemObjectError::NotAValidId),
        3 => Err(RequestSystemObjectError::AccessDenied),

        _ => panic!("Syscall request_system_object returned unexpected status code"),
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
