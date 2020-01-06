use super::{raw, result, result::define_error_type, SYSCALL_REQUEST_SYSTEM_OBJECT};
use crate::KernelObjectId;
use core::convert::TryFrom;

pub const SYSTEM_OBJECT_BACKUP_FRAMEBUFFER_ID: usize = 0;

#[derive(Clone, Copy, Debug)]
pub enum SystemObjectId {
    BackupFramebuffer { info_address: *mut FramebufferSystemObjectInfo },
}

define_error_type!(RequestSystemObjectError {
    /// The requested object ID does point to a valid system object, but the kernel has not created
    /// a corresponding object for it.
    ObjectDoesNotExist => 1,

    /// The requested object ID does not correspond to a valid system object.
    NotAValidId => 2,

    /// The requested object ID is valid, but the requesting task does not have the correct
    /// capabilities to access it.
    AccessDenied => 3,
});

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
    let result = match id {
        SystemObjectId::BackupFramebuffer { info_address } => unsafe {
            raw::syscall2(
                SYSCALL_REQUEST_SYSTEM_OBJECT,
                SYSTEM_OBJECT_BACKUP_FRAMEBUFFER_ID,
                info_address as usize,
            )
        },
    };

    result::result_from_syscall_repr(result)
}
