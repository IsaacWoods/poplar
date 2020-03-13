//! The HAL memory API tries to model an abstract version of a sensible architecture's virtual memory model. It
//! should be suitable for implementations for x86_64, AArch64, and paged RISC-V.
//!
//! It assumes two address spaces, the physical and the virtual (each of which have an address type). Physical
//! memory is split into frames, while virtual memory is split into pages. A `Mapper` can be used to map parts of
//! the virtual address space into the physical address space.

mod frame;
mod page;
mod physical_address;
mod virtual_address;

pub use frame::Frame;
pub use page::Page;
pub use physical_address::PhysicalAddress;
pub use virtual_address::VirtualAddress;

use core::fmt::Debug;

/// Multiply by this to turn KiB into bytes
pub const KIBIBYTES_TO_BYTES: usize = 1024;
/// Multiply by this to turn MiB into bytes
pub const MEBIBYTES_TO_BYTES: usize = 1024 * KIBIBYTES_TO_BYTES;
/// Multiply by this to turn GiB into bytes
pub const GIBIBYTES_TO_BYTES: usize = 1024 * MEBIBYTES_TO_BYTES;

/// This trait is implemented by a number of marker types, one for each size of frame and page. Not all sizes may
/// be available on the target architecture.
pub trait FrameSize: Clone + Copy + PartialEq + Eq + PartialOrd + Ord + Debug {
    /// Frame size in bytes
    const SIZE: usize;
}

macro frame_size($name: ident, $size: expr, $condition: meta) {
    #[$condition]
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
    pub enum $name {}

    #[$condition]
    impl FrameSize for $name {
        const SIZE: usize = $size;
    }
}

frame_size!(Size4KiB, 4 * KIBIBYTES_TO_BYTES, cfg(target_arch = "x86_64"));
frame_size!(Size2MiB, 2 * MEBIBYTES_TO_BYTES, cfg(target_arch = "x86_64"));
frame_size!(Size1GiB, 1 * GIBIBYTES_TO_BYTES, cfg(target_arch = "x86_64"));
