//! This crate provides a platform-agnostic interface between the loader and the kernel, providing information such
//! as the memory map and chosen video mode. Not all of this information will be passed on every architecture.

#![no_std]

use core::ops::Range;

#[repr(C)]
pub struct BootInfo<'a> {
    /// Map of available memory that the kernel. This only includes ranges of memory that can be freely used at
    /// some point, and so memory used for e.g. UEFI runtime services are simply not included. The kernel must
    /// assume that memory not features in this map is not available for use.
    pub memory_map: &'a [MemoryMapEntry],
    pub loaded_images: &'a [LoadedImage],
    pub video_mode: Option<VideoModeInfo>,
}

#[repr(C)]
pub enum MemoryType {
    /// Memory that can be used freely by the OS.
    Conventional,

    /// Memory that contains ACPI tables. After the OS has finished with them, this may be treated as conventional
    /// memory.
    AcpiReclaimable,

    /// Memory occupied by images that the loader has been asked to load from disk. If the kernel can determine
    /// that an image is no longer needed, it may use this memory.
    LoadedImage,

    /// Memory that is occupied by page tables created by the loader for the kernel. If the kernel can determine
    /// that it no longer needs part of this mapped, it may use this memory.
    KernelPageTables,

    /// Memory that has been mapped for the kernel heap.
    KernelHeap,

    /// Memory that is occupied by the boot info constructed by the loader for the kernel. Contains the `BootInfo`
    /// structure, and all the structures that are referenced by it. After the kernel has finished with this data,
    /// it may use this memory.
    BootInfo,
}

#[repr(C)]
pub struct MemoryMapEntry {
    pub range: Range<usize>,
    pub memory_type: MemoryType,
}

/// This is one less than a power-of-two, because then it's aligned when placed after the length byte.
pub const MAX_IMAGE_NAME_LENGTH: usize = 31;
pub const MAX_CAPABILITY_STREAM_LENGTH: usize = 32;

/// Describes an image loaded from the filesystem by the loader, as the kernel does not have the capabilities to do
/// so. Images are expected to have three segments (`rodata` loaded as read-only, `data` loaded as read+write, and
/// `text` loaded as read+execute).
#[repr(C)]
pub struct LoadedImage {
    pub name_length: u8,
    /// The bytes of the image's name, encoded as UTF-8. Not null-terminated.
    pub name: [u8; MAX_IMAGE_NAME_LENGTH],
    pub text: Segment,
    pub data: Segment,
    pub rodata: Segment,
    /// The virtual address at which to start executing the image.
    pub entry_point: usize,
    pub capability_stream: [u8; MAX_CAPABILITY_STREAM_LENGTH],
}

#[repr(C)]
pub struct Segment {
    pub physical_address: usize,
    pub virtual_address: usize,
    /// In bytes.
    pub size: usize,
}

#[repr(C)]
pub struct VideoModeInfo {
    pub framebuffer_address: usize,
    pub pixel_format: PixelFormat,
    pub width: usize,
    pub height: usize,
    /// The number of pixels in each scan-line. May be greater than `width`.
    pub stride: usize,
}

#[repr(C)]
pub enum PixelFormat {
    /// Each pixel is represented by 4 bytes, with the layout:
    /// |--------|--------|--------|--------|
    /// | ------ | blue   | green  | red    |
    /// |--------|--------|--------|--------|
    RGB32,

    /// Each pixel is represented by 4 bytes, with the layout:
    /// |--------|--------|--------|--------|
    /// | ------ | red    | green  | blue   |
    /// |--------|--------|--------|--------|
    BGR32,
}
