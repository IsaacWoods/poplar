pub mod frame;
pub mod frame_allocator;
pub mod kernel_map;
pub mod page;
pub mod page_table;
pub mod physical_address;
pub mod virtual_address;

pub use self::{
    frame::Frame,
    frame_allocator::FrameAllocator,
    page::Page,
    page_table::{EntryFlags, MapError, Mapper, PageTable, TranslationResult},
    physical_address::PhysicalAddress,
    virtual_address::VirtualAddress,
};

/// Multiply by this to turn KiB into bytes
pub const KIBIBYTES_TO_BYTES: usize = 1024;
/// Multiply by this to turn MiB into bytes
pub const MEBIBYTES_TO_BYTES: usize = 1024 * KIBIBYTES_TO_BYTES;
/// Multiply by this to turn GiB into bytes
pub const GIBIBYTES_TO_BYTES: usize = 1024 * MEBIBYTES_TO_BYTES;

/// Implemented by marker types that denote the various sizes of frames and pages. Despite the
/// name, this is used by both `Frame` and `Page`.
pub trait FrameSize: Clone + Copy + PartialEq + Eq + PartialOrd + Ord {
    /// Frame size in bytes
    const SIZE: usize;

    /// The log2 of the frame size (in bytes). This makes some maths involving frame and page
    /// addresses more efficient.
    const LOG2_SIZE: usize;
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Size4KiB {}

impl FrameSize for Size4KiB {
    const SIZE: usize = 4 * KIBIBYTES_TO_BYTES;
    const LOG2_SIZE: usize = 12;
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Size2MiB {}

impl FrameSize for Size2MiB {
    const SIZE: usize = 2 * MEBIBYTES_TO_BYTES;
    const LOG2_SIZE: usize = 21;
}
