#![no_std]

/// On x86_64 with 4-level paging, the higher-half starts at `0xffff_8000_0000_0000`. We dedicate
/// the first half of the higher-half (64 TiB) to the direct physical map. Following this is an
/// area the kernel can use for dynamic virtual allocations (starting at `0xffff_c000_0000_0000`).
///
/// The actual kernel image is loaded at `-2GiB` (`0xffff_ffff_8000_0000`), and is followed by boot
/// information constructed by Seed. This allows best utilisation of the `kernel` code model, which
/// optimises for encoding offsets in signed 32-bit immediates, which are common in x86_64 instruction
/// encodings.
#[cfg(target_arch = "x86_64")]
pub mod kernel_map {
    use hal::memory::VAddr;

    pub const HIGHER_HALF_START: VAddr = VAddr::new(0xffff_8000_0000_0000);
    pub const PHYSICAL_MAPPING_BASE: VAddr = HIGHER_HALF_START;
    pub const KERNEL_DYNAMIC_AREA_BASE: VAddr = VAddr::new(0xffff_c000_0000_0000);
    pub const KERNEL_IMAGE_BASE: VAddr = VAddr::new(0xffff_ffff_8000_0000);
}

pub const MAGIC: u32 = 0xf0cacc1a;

// TODO: framebuffer, seed version, kernel config, user task configs(?)
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Header {
    pub magic: u32,

    /// Offset from the start of this header to the memory map.
    pub mem_map_offset: u16,
    /// Length of the memory map, in entries.
    pub mem_map_length: u16,

    /// The **virtual** address, after the kernel and boot info, at which the kernel can start to dynamically allocate memory.
    pub kernel_free_start: u64,

    /// The physical address of the RSDP, if found. If not, this will be `0`.
    pub rsdp_address: u64,

    /// The physical address of the device tree, if found. If not, this will be `0`.
    pub device_tree_address: u64,

    pub loaded_images_offset: u16,
    pub num_loaded_images: u16,

    pub string_table_offset: u16,
    pub string_table_length: u16,

    /// Offset from the start of this header to the `VideoModeInfo` descriptor, if one is present. Otherwise `0`.
    pub video_mode_offset: u16,
    pub _reserved0: [u16; 3],
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[repr(u32)]
pub enum MemoryType {
    /// Memory usable by the kernel. This includes memory previously occupied by Seed.
    Usable,

    /// Memory marked as reserved by the firmware. The kernel should not use this memory.
    Reserved,

    /// Memory used by the ACPI tables. If the kernel does not need further use of the tables, it may use this memory.
    AcpiReclaimable,

    /// Memory marked as ACPI NVS by the firmware.
    AcpiNvs,

    /// Memory used be the UEFI Runtime Services. The kernel should not use this data.
    UefiRuntimeServices,

    /// Memory used by the kernel's loaded image and the kernel's page tables.
    Kernel,

    /// Memory used by other images loaded by the bootloader.
    LoadedImage,

    /// Memory used by a framebuffer.
    Framebuffer,

    /// Scratch entries are placed at the end of the memory map, and facilitate memory map manipulation by the kernel for the purposes of early memory allocation.
    /// They should be ignored for all other purposes.
    Scratch,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct MemoryEntry {
    pub base: u64,
    pub length: u64,
    pub typ: MemoryType,
    pub _reserved: u32,
}

pub const LOADED_IMAGE_MAX_SEGMENTS: usize = 3;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct LoadedImage {
    pub name_offset: u16,
    pub name_len: u16,
    pub num_segments: u16,
    pub _reserved0: u16,
    pub segments: [LoadedSegment; LOADED_IMAGE_MAX_SEGMENTS],
    pub entry_point: u64,
}

#[derive(Clone, Copy, Default, Debug)]
#[repr(C)]
pub struct LoadedSegment {
    pub phys_addr: u64,
    pub virt_addr: u64,
    pub size: u32,
    pub flags: SegmentFlags,
}

mycelium_bitfield::bitfield! {
    #[derive(Default)]
    pub struct SegmentFlags<u32> {
        pub const WRITABLE: bool;
        pub const EXECUTABLE: bool;
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct VideoModeInfo {
    /// The physical address of the framebuffer
    pub framebuffer_address: u64,
    pub pixel_format: PixelFormat,
    pub width: u64,
    pub height: u64,
    /// The number of pixels in each scan-line. May be greater than `width`.
    pub stride: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u64)]
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
