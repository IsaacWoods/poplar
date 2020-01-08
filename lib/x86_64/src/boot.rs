use crate::memory::{page_table::EntryFlags, Frame, PhysicalAddress, VirtualAddress};
use core::{fmt, ops::Range};

pub const BOOT_INFO_MAGIC: u32 = 0xcafebabe;
pub const NUM_MEMORY_MAP_ENTRIES: usize = 64;
pub const NUM_IMAGES: usize = 16;
/// Each initial image is expected to have a maximum of three segments: read-only, read+write,
/// and read+execute.
pub const NUM_SEGMENTS_PER_IMAGE: usize = 3;
pub const MAX_CAPABILITY_BYTES_PER_IMAGE: usize = 32;
/// The maximum number of bytes that the task's name can be encoded as UTF-8 in. Must not be
/// greater than 256.
pub const MAX_NAME_BYTES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryType {
    /// Memory used by the UEFI services. Cannot be used by the OS.
    UefiServices,

    /// Conventional memory that can freely be used by the OS,
    Conventional,

    /// Memory that contains ACPI tables. After the OS has parsed the ACPI tables, it can use this
    /// memory as if it was `Conventional`.
    AcpiReclaimable,

    /// This marks memory that the OS should preserve in the working and S1-S3 sleep states.
    SleepPreserve,

    /// This marks memory that the OS should preserve in the working and S1-S4 sleep states.
    NonVolatileSleepPreserve,

    /// Memory the bootloader has mapped the kernel image into. The OS should not use it, or it
    /// will corrupt its own code or data.
    KernelImage,

    /// Memory the bootloader has mapped images its been asked to load into.
    LoadedImage,

    /// Memory the bootloader has used for the page tables containing the kernel's mapping. The OS
    /// should not use this memory, unless it has permanently switched to another set of page
    /// tables. This also includes memory used for the payload's image.
    KernelPageTables,

    /// Memory the bootloader has mapped for use as the kernel heap. The OS should not use this
    /// memory, except as heap space.
    KernelHeap,

    /// Memory used for storing the `BootInfo` by the bootloader. It can be used by the OS after it
    /// has finished with the information passed to it from the bootloader.
    BootInfo,
}

#[derive(Debug)]
#[repr(C)]
pub struct MemoryEntry {
    pub area: Range<Frame>,
    pub memory_type: MemoryType,
}

impl Default for MemoryEntry {
    fn default() -> Self {
        MemoryEntry {
            area: Frame::contains(PhysicalAddress::new(0x0).unwrap())
                ..(Frame::contains(PhysicalAddress::new(0x0).unwrap())),
            memory_type: MemoryType::UefiServices,
        }
    }
}

/// Describes a memory region that should be represented by the `MemoryObject` kernel object in the
/// kernel.
#[derive(Clone, Copy, Default, Debug)]
#[repr(C)]
pub struct MemoryObjectInfo {
    pub physical_address: PhysicalAddress,
    pub virtual_address: VirtualAddress,
    /// Size in bytes.
    pub size: usize,
    pub permissions: EntryFlags,
}

/// An image loaded from the filesystem by the bootloader. The kernel should turn this information
/// into the correct representation and treat this image like a normal task.
#[derive(Clone, Copy, Default, Debug)]
#[repr(C)]
pub struct ImageInfo {
    /// The name of the name in bytes. Maximum of 32.
    pub name_length: u8,
    /// Name of the task that this image represents. Must be valid UTF-8 and a maximum of 32 bytes.
    pub name: [u8; 32],
    pub num_segments: usize,
    pub segments: [MemoryObjectInfo; NUM_SEGMENTS_PER_IMAGE],
    pub entry_point: VirtualAddress,
    pub capability_stream: [u8; MAX_CAPABILITY_BYTES_PER_IMAGE],
}

impl ImageInfo {
    /// This should only be called from the bootloader.
    pub fn add_segment(&mut self, segment: MemoryObjectInfo) {
        if self.num_segments == NUM_SEGMENTS_PER_IMAGE {
            panic!("Run out of space for segments in the ImageInfo!");
        }

        self.segments[self.num_segments] = segment;
        self.num_segments += 1;
    }

    pub fn segments(&self) -> &[MemoryObjectInfo] {
        &self.segments[0..self.num_segments]
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum PixelFormat {
    /// Each pixel is represented by 4 bytes, with the following layout:
    /// |--------|--------|--------|--------|
    /// | ------ | blue   | green  | red    |
    /// |--------|--------|--------|--------|
    RGB32,

    /// Each pixel is represented by 4 bytes, with the following layout:
    /// |--------|--------|--------|--------|
    /// | ------ | red    | green  | blue   |
    /// |--------|--------|--------|--------|
    BGR32,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct VideoInfo {
    pub framebuffer_address: PhysicalAddress,
    pub pixel_format: PixelFormat,
    pub width: u32,
    pub height: u32,
    /// How many pixels are in each scan-line. This can be greater than `width`.
    pub stride: u32,
}

/// This structure is placed in memory by the bootloader and a reference to it passed to the
/// kernel. It allows the kernel to access information discovered by the bootloader, such as the
/// graphics mode it switched to.
///
/// The memory map only contains regions for **usable** memory. If a frame does not appear in the
/// memory map, it can not be used by the OS at any stage.
///
/// It is marked `repr(C)` to give it a standarised layout. If we used Rust's layout, and built the
/// bootloader and kernel with different compilers, the kernel could expect a different layout to
/// the one the bootloader has laid out in memory. If Rust ever settles on a standard ABI, this can
/// be removed.
#[repr(C)]
pub struct BootInfo {
    /// This should be set to `BOOT_INFO_MAGIC` by the bootloader.
    pub magic: u32,
    pub memory_map: [MemoryEntry; NUM_MEMORY_MAP_ENTRIES],
    pub num_memory_map_entries: usize,
    pub rsdp_address: Option<PhysicalAddress>,
    pub num_images: usize,
    pub images: [ImageInfo; NUM_IMAGES],
    pub video_info: Option<VideoInfo>,
}

impl fmt::Debug for BootInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut foo = f.debug_struct("BootInfo");
        foo.field("magic", &self.magic);
        foo.field("num_memory_map_entries", &self.num_memory_map_entries);
        for i in 0..self.num_memory_map_entries {
            foo.field("memory_map_entry", &self.memory_map[i]);
        }
        foo.field("rsdp_address", &self.rsdp_address);
        foo.field("num_images", &self.num_images);
        for i in 0..self.num_images {
            foo.field("image", &self.images[i]);
        }
        foo.field("video_info", &self.video_info);
        foo.finish()
    }
}

impl BootInfo {
    pub fn memory_entries(&self) -> &[MemoryEntry] {
        &self.memory_map[0..self.num_memory_map_entries]
    }

    /// This should only be called from the bootloader.
    pub fn add_memory_map_entry(&mut self, entry: MemoryEntry) {
        if self.num_memory_map_entries == NUM_MEMORY_MAP_ENTRIES {
            panic!("Run out of space for memory map entries in the BootInfo!");
        }

        self.memory_map[self.num_memory_map_entries] = entry;
        self.num_memory_map_entries += 1;
    }

    pub fn images(&self) -> &[ImageInfo] {
        &self.images[0..self.num_images]
    }

    /// This should only be called from the bootloader.
    pub fn add_image(&mut self, image: ImageInfo) {
        if self.num_images == NUM_IMAGES {
            panic!("Run out of space for loaded images in the BootInfo!");
        }

        self.images[self.num_images] = image;
        self.num_images += 1;
    }
}
