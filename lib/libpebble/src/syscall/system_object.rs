use super::{raw, result, SYSCALL_REQUEST_SYSTEM_OBJECT};
use crate::KernelObjectId;

pub const SYSTEM_OBJECT_BACKUP_FRAMEBUFFER_ID: usize = 0;

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

// TODO: this shouldn't be able to fail. I guess this should be TryFrom (maybe with Error = ())?
impl From<u32> for RequestSystemObjectError {
    fn from(status: u32) -> Self {
        match status {
            0 => panic!("Tried to construct RequestSystemObjectError from successful status!"),
            1 => RequestSystemObjectError::ObjectDoesNotExist,
            2 => RequestSystemObjectError::NotAValidId,
            3 => RequestSystemObjectError::AccessDenied,
            _ => panic!("Tried to construct RequestSystemObjectError from invalid status!"),
        }
    }
}

impl Into<u32> for RequestSystemObjectError {
    fn into(self) -> u32 {
        match self {
            RequestSystemObjectError::ObjectDoesNotExist => 1,
            RequestSystemObjectError::NotAValidId => 2,
            RequestSystemObjectError::AccessDenied => 3,
        }
    }
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
