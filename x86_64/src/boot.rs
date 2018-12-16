use crate::memory::paging::Frame;
use core::ops::Range;

pub const BOOT_INFO_MAGIC: u32 = 0xcafebabe;
pub const MEMORY_MAP_NUM_ENTRIES: usize = 64;

#[derive(Debug)]
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

    /// Memory the bootloader has used for the page tables containing the kernel's mapping. The OS
    /// should not use this memory, unless it has permanently switched to another set of page
    /// tables.
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
    pub memory_map: [MemoryEntry; MEMORY_MAP_NUM_ENTRIES],
    pub num_memory_map_entries: usize,
}
