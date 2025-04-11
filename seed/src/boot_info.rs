//! The "boot info" refers to a data structure passed from Seed to the kernel, telling it about the platform it's
//! running on, memory it can use, and about other objects Seed has been asked to load into memory.
//!
//! Seed implementations generally don't have their own heaps, and so these data structures need to be
//! representable without heap allocation. For this reason, the `heapless` crate is used to supply stack-backed
//! containers - the resulting data structure is then serialized using `ptah`, and can then be deserialized in the
//! kernel.

// TODO: it feels dubious to me to use `heapless` here for some reason - their layouts seem fine
// but are not `repr(C)`, and I also wonder if a string table might be a better idea idk?

use core::{fmt, ops::Range};
use hal::memory::{Bytes, Flags, Frame, PAddr, VAddr};
use heapless::{String, Vec};

pub const BOOT_INFO_MAGIC: u32 = 0xf0cacc1a;
pub const MAX_MEMORY_MAP_ENTRIES: usize = 256;
pub const MAX_LOADED_IMAGES: usize = 32;
pub const MAX_IMAGE_NAME_LENGTH: usize = 32;
pub const MAX_IMAGE_LOADED_SEGMENTS: usize = 3;

pub type MemoryMap = Vec<MemoryMapEntry, MAX_MEMORY_MAP_ENTRIES>;

#[derive(Default, Debug)]
#[repr(C)]
pub struct BootInfo {
    pub magic: u32,

    /// Map of available memory that the kernel. This only includes ranges of memory that can be freely used at
    /// some point, and so memory used for e.g. UEFI runtime services are simply not included. The kernel must
    /// assume that memory not featured in this map is not available for use.
    // TODO: maybe we should include all memory in the memory map? Like why not tell the kernel
    // that stuff?
    pub memory_map: MemoryMap,

    pub loaded_images: Vec<LoadedImage, MAX_LOADED_IMAGES>,
    pub video_mode: Option<VideoModeInfo>,
    pub heap_address: VAddr,
    pub heap_size: usize,

    /// The physical address of the RSDP, the first ACPI table, if one is present.
    pub rsdp_address: Option<PAddr>,

    /// The physical address of the device tree, if one is present.
    pub fdt_address: Option<PAddr>,
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
#[repr(C)]
pub enum MemoryType {
    /// Memory that can be used freely by the kernel.
    #[default]
    Conventional,

    /// Memory that contains ACPI tables. After the OS has finished with them, this may be treated as conventional
    /// memory.
    AcpiReclaimable,

    /// Memory that contains the Flattened Device Tree (FDT). After the OS has finished with the device tree, this
    /// memory can be treated as conventional.
    FdtReclaimable,

    /// Memory occupied by images that the loader has been asked to load from disk. If the kernel can determine
    /// that an image is no longer needed, it may use this memory.
    LoadedImage,

    /// Memory that is occupied by page tables created by the loader for the kernel. If the kernel can determine
    /// that it no longer needs part of this mapped, it may use this memory.
    KernelPageTables,

    /// Memory that has been mapped for the kernel heap.
    KernelHeap,

    /// Memory that the loader maps into the kernel address space. It may be reclaimed by the kernel immediately,
    /// and the kernel should also unmap it from its address space. Seed will only produce these entries if the
    /// implementation needs to keep itself mapped - otherwise this memory may be marked `Conventional`.
    Loader,

    /// Memory that is occupied by the boot info constructed by the loader for the kernel. Contains the `BootInfo`
    /// structure, and all the structures that are referenced by it. After the kernel has finished with this data,
    /// it may use this memory.
    BootInfo,
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct MemoryMapEntry {
    pub typ: MemoryType,
    pub start: PAddr,
    pub size: Bytes,
}

impl MemoryMapEntry {
    pub fn new(typ: MemoryType, start: PAddr, size: Bytes) -> MemoryMapEntry {
        MemoryMapEntry { typ, start, size }
    }

    pub fn address_range(&self) -> Range<PAddr> {
        self.start..(self.start + self.size)
    }

    pub fn frame_range(&self) -> Range<Frame> {
        Frame::starts_with(self.start)..Frame::starts_with(self.start + self.size)
    }
}

impl fmt::Debug for MemoryMapEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:#x?}..{:#x?}) => {:?}", self.start, self.start + self.size, self.typ)
    }
}

/// Describes an image loaded from the filesystem by the loader, as the kernel does not have the capabilities to do
/// so. Images are expected to have three segments (`rodata` loaded as read-only, `data` loaded as read+write, and
/// `text` loaded as read+execute).
#[derive(Clone, Default, Debug)]
#[repr(C)]
pub struct LoadedImage {
    pub name: String<MAX_IMAGE_NAME_LENGTH>,
    pub segments: Vec<Segment, MAX_IMAGE_LOADED_SEGMENTS>,
    pub master_tls: Option<Segment>,
    /// The virtual address at which to start executing the image.
    pub entry_point: VAddr,
}

#[derive(Clone, Copy, Default, Debug)]
#[repr(C)]
pub struct Segment {
    pub physical_address: PAddr,
    pub virtual_address: VAddr,
    pub size: Bytes,
    pub flags: Flags,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct VideoModeInfo {
    pub framebuffer_address: PAddr,
    pub pixel_format: PixelFormat,
    pub width: usize,
    pub height: usize,
    /// The number of pixels in each scan-line. May be greater than `width`.
    pub stride: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(C)]
pub enum PixelFormat {
    /// Each pixel is represented by 4 bytes, with the layout:
    /// |--------|--------|--------|--------|
    /// | ------ | blue   | green  | red    |
    /// |--------|--------|--------|--------|
    Rgb32,

    /// Each pixel is represented by 4 bytes, with the layout:
    /// |--------|--------|--------|--------|
    /// | ------ | red    | green  | blue   |
    /// |--------|--------|--------|--------|
    Bgr32,
}
