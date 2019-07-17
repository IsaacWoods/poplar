use crate::uefi::{boot_services::Protocol, Guid, Status};
use core::{mem, ptr};

static GOP_GUID: Guid =
    Guid { a: 0x9042a9de, b: 0x23dc, c: 0x4a38, d: [0x96, 0xfb, 0x7a, 0xde, 0xd0, 0x80, 0x51, 0x6a] };

#[repr(C)]
pub struct GraphicsOutput {
    query_mode: extern "win64" fn(&Self, mode: u32, info_size: &mut usize, &mut *const ModeInfo) -> Status,
    set_mode: extern "win64" fn(&mut Self, mode: u32) -> Status,
    blt: unsafe extern "win64" fn(
        &mut Self,
        buffer: *mut BltPixel,
        op: u32,
        source_x: usize,
        source_y: usize,
        dest_x: usize,
        dest_y: usize,
        width: usize,
        height: usize,
        stride: usize,
    ) -> Status,
    mode: *const ModeData,
}

pub struct ModesIter<'a> {
    gop: &'a GraphicsOutput,
    current: u32,
    max: u32,
}

impl<'a> Iterator for ModesIter<'a> {
    type Item = (u32, ModeInfo);

    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.max {
            let index = self.current;
            self.current += 1;
            self.gop.query_mode(index).map(|info| (index, info)).ok()
        } else {
            None
        }
    }
}

impl GraphicsOutput {
    pub fn query_mode(&self, index: u32) -> Result<ModeInfo, Status> {
        let mut info_size = 0;
        let mut info = ptr::null();

        (self.query_mode)(self, index, &mut info_size, &mut info).as_result().map(|_| {
            assert_eq!(info_size, mem::size_of::<ModeInfo>());
            unsafe { *info }
        })
    }

    /// Iterate over all the modes supported by this device, as pairs of the index and the
    /// `ModeInfo` for each mode.
    pub fn modes(&self) -> ModesIter<'_> {
        ModesIter { gop: &self, current: 0, max: unsafe { (*self.mode).max_mode } }
    }

    pub fn set_mode(&mut self, index: u32) -> Result<(), Status> {
        (self.set_mode)(self, index).as_result().map(|_| ())
    }

    pub fn mode_data(&self) -> ModeData {
        assert!(self.mode != ptr::null());
        unsafe { *self.mode }
    }
}

impl Protocol for GraphicsOutput {
    fn guid() -> &'static Guid {
        &GOP_GUID
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct ModeData {
    pub max_mode: u32,
    pub mode: u32,
    pub current_mode_info: *const ModeInfo,
    /// Size of the above structure.
    pub current_mode_info_size: usize,
    /// Physical address of the framebuffer.
    pub framebuffer_address: u64,
    /// Size of the framebuffer in bytes. Should be equal to `{size of a pixel} * height * stride`.
    pub framebuffer_size: usize,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[allow(dead_code)] // We don't construct any of these variants manually, so we supress those warnings
#[repr(u32)]
pub enum PixelFormat {
    /// Each pixel is 32-bit long, with 24-bit RGB, and the last byte is reserved.
    RGB = 0,
    /// Each pixel is 32-bit long, with 24-bit BGR, and the last byte is reserved.
    BGR = 1,
    /// Custom pixel format, described by the pixel bitmask.
    Bitmask = 2,
    /// The graphics mode does not support drawing directly to the frame buffer.
    ///
    /// This means you will have to use the `blt` function which will
    /// convert the graphics data to the device's internal pixel format.
    BltOnly = 3,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(C)]
pub struct PixelBitmask {
    /// The bits indicating the red channel.
    pub red: u32,
    /// The bits indicating the green channel.
    pub green: u32,
    /// The bits indicating the blue channel.
    pub blue: u32,
    /// The reserved bits, which are ignored by the video hardware.
    pub reserved: u32,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct ModeInfo {
    pub version: u32,
    pub x_resolution: u32,
    pub y_resolution: u32,
    pub format: PixelFormat,
    pub mask: PixelBitmask,
    /// Number of pixels per video memory line. For performance reasons, this can be greater than
    /// the width to align each line.
    pub stride: u32,
}

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct BltPixel {
    pub blue: u8,
    pub green: u8,
    pub red: u8,
    _reserved: u8,
}
