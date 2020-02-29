#![no_std]

pub mod kernel_map;

use core::ops::Range;
use x86_64::memory::{Frame, PhysicalAddress, VirtualAddress};

pub const BOOT_INFO_MAGIC: u32 = 0xcafebabe;

#[derive(Default)]
#[repr(C)]
pub struct BootInfo {
    pub magic: u32,
    /// Map of available memory that the kernel. This only includes ranges of memory that can be freely used at
    /// some point, and so memory used for e.g. UEFI runtime services are simply not included. The kernel must
    /// assume that memory not features in this map is not available for use.
    pub memory_map: MemoryMap,
    pub loaded_images: LoadedImages,
    pub video_mode: Option<VideoModeInfo>,
    pub heap_address: VirtualAddress,
    pub heap_size: usize,

    /// The physical address of the RSDP, the first ACPI table.
    pub rsdp_address: Option<PhysicalAddress>,
}

pub const MAX_MEMORY_MAP_ENTRIES: usize = 64;

#[derive(Clone)]
#[repr(C)]
pub struct MemoryMap {
    pub num_entries: u8,
    pub entries: [MemoryMapEntry; MAX_MEMORY_MAP_ENTRIES],
}

impl MemoryMap {
    pub fn add_entry(&mut self, entry: MemoryMapEntry) -> Result<(), ()> {
        if (self.num_entries as usize) >= MAX_MEMORY_MAP_ENTRIES {
            return Err(());
        }

        self.entries[self.num_entries as usize] = entry;
        self.num_entries += 1;
        Ok(())
    }

    pub fn entries(&self) -> &[MemoryMapEntry] {
        &self.entries[0..(self.num_entries as usize)]
    }
}

impl Default for MemoryMap {
    fn default() -> Self {
        MemoryMap { num_entries: 0, entries: [Default::default(); MAX_MEMORY_MAP_ENTRIES] }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct MemoryMapEntry {
    pub start: PhysicalAddress,
    pub size: usize,
    pub memory_type: MemoryType,
}

impl MemoryMapEntry {
    pub fn address_range(&self) -> Range<PhysicalAddress> {
        self.start..(self.start + self.size)
    }

    pub fn frame_range(&self) -> Range<Frame> {
        Frame::starts_with(self.start)..Frame::starts_with(self.start + self.size)
    }
}

impl Default for MemoryMapEntry {
    fn default() -> Self {
        MemoryMapEntry {
            start: PhysicalAddress::new(0x0).unwrap(),
            size: 0,
            memory_type: MemoryType::Conventional,
        }
    }
}

pub const MAX_LOADED_IMAGES: usize = 256;

#[repr(C)]
pub struct LoadedImages {
    pub num_images: u8,
    pub images: [LoadedImage; MAX_LOADED_IMAGES],
}

impl LoadedImages {
    pub fn images(&self) -> &[LoadedImage] {
        &self.images[0..self.num_images as usize]
    }

    pub fn add_image(&mut self, image: LoadedImage) -> Result<(), ()> {
        if self.num_images as usize >= MAX_LOADED_IMAGES {
            return Err(());
        }

        self.images[self.num_images as usize] = image;
        self.num_images += 1;
        Ok(())
    }
}

impl Default for LoadedImages {
    fn default() -> Self {
        LoadedImages { num_images: 0, images: [Default::default(); MAX_LOADED_IMAGES] }
    }
}

/// This is one less than a power-of-two, because then it's aligned when placed after the length byte.
pub const MAX_IMAGE_NAME_LENGTH: usize = 31;
pub const MAX_IMAGE_LOADED_SEGMENTS: usize = 3;
pub const MAX_CAPABILITY_STREAM_LENGTH: usize = 32;

/// Describes an image loaded from the filesystem by the loader, as the kernel does not have the capabilities to do
/// so. Images are expected to have three segments (`rodata` loaded as read-only, `data` loaded as read+write, and
/// `text` loaded as read+execute).
#[derive(Clone, Copy, Default, Debug)]
#[repr(C)]
pub struct LoadedImage {
    pub name_length: u8,
    /// The bytes of the image's name, encoded as UTF-8. Not null-terminated.
    pub name: [u8; MAX_IMAGE_NAME_LENGTH],
    pub num_segments: u8,
    pub segments: [Segment; MAX_IMAGE_LOADED_SEGMENTS],
    /// The virtual address at which to start executing the image.
    pub entry_point: VirtualAddress,
    pub capability_stream: [u8; MAX_CAPABILITY_STREAM_LENGTH],
}

impl LoadedImage {
    pub fn add_segment(&mut self, segment: Segment) -> Result<(), ()> {
        if self.num_segments as usize == MAX_IMAGE_LOADED_SEGMENTS {
            return Err(());
        }

        self.segments[self.num_segments as usize] = segment;
        self.num_segments += 1;
        Ok(())
    }

    pub fn segments(&self) -> &[Segment] {
        &self.segments[0..(self.num_segments as usize)]
    }
}

#[derive(Clone, Copy, Default, Debug)]
#[repr(C)]
pub struct Segment {
    pub physical_address: PhysicalAddress,
    pub virtual_address: VirtualAddress,
    /// In bytes.
    pub size: usize,
    pub writable: bool,
    pub executable: bool,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct VideoModeInfo {
    pub framebuffer_address: PhysicalAddress,
    pub pixel_format: PixelFormat,
    pub width: usize,
    pub height: usize,
    /// The number of pixels in each scan-line. May be greater than `width`.
    pub stride: usize,
}

#[derive(Clone, Copy, Debug)]
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
