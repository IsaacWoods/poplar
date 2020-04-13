use super::{
    raw,
    result::{define_error_type, handle_from_syscall_repr},
    SYSCALL_GET_FRAMEBUFFER,
};
use crate::Handle;

define_error_type!(GetFramebufferError {
    /// The calling task does not have the correct capability to access the framebuffer.
    AccessDenied => 1,

    /// The address passed in `a` to write the info struct into was invalid.
    InfoAddressIsInvalid => 2,

    /// The kernel did not create a framebuffer.
    NoFramebufferCreated => 3,
});

/// Describes how the supplied framebuffer represents pixels.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum PixelFormat {
    RGB32 = 0,
    BGR32 = 1,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct FramebufferInfo {
    pub width: u16,
    pub height: u16,
    pub stride: u16,
    pub pixel_format: PixelFormat,
}

pub fn get_framebuffer(info: *mut FramebufferInfo) -> Result<Handle, GetFramebufferError> {
    handle_from_syscall_repr(unsafe { raw::syscall1(SYSCALL_GET_FRAMEBUFFER, info as usize) })
}
