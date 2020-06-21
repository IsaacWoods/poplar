//! The HAL memory API tries to model an abstract version of a sensible architecture's virtual memory model. It
//! should be suitable for implementations for x86_64, AArch64, and paged RISC-V.
//!
//! It assumes two address spaces, the physical and the virtual (each of which have an address type). Physical
//! memory is split into frames, while virtual memory is split into pages. A `Mapper` can be used to map parts of
//! the virtual address space into the physical address space.

mod frame;
mod page;
mod paging;
mod physical_address;
mod virtual_address;

pub use frame::Frame;
pub use page::Page;
pub use paging::{Flags, PageTable, PagingError};
pub use physical_address::PhysicalAddress;
pub use virtual_address::VirtualAddress;

use core::{fmt::Debug, ops::Range};

pub type Bytes = usize;
pub type Kibibytes = usize;
pub type Mebibytes = usize;
pub type Gibibytes = usize;

pub const fn kibibytes(kibibytes: Kibibytes) -> Bytes {
    kibibytes * 1024
}

pub const fn mebibytes(mebibytes: Mebibytes) -> Bytes {
    kibibytes(mebibytes * 1024)
}

pub const fn gibibytes(gibibytes: Gibibytes) -> Bytes {
    mebibytes(gibibytes * 1024)
}

// /// Multiply by this to turn KiB into bytes
// pub const KIBIBYTES_TO_BYTES: usize = 1024;
// /// Multiply by this to turn MiB into bytes
// pub const MEBIBYTES_TO_BYTES: usize = 1024 * KIBIBYTES_TO_BYTES;
// /// Multiply by this to turn GiB into bytes
// pub const GIBIBYTES_TO_BYTES: usize = 1024 * MEBIBYTES_TO_BYTES;

/// This trait is implemented by a number of marker types, one for each size of frame and page. Different size
/// types are defined depending on the target architecture.
pub trait FrameSize: Clone + Copy + PartialEq + Eq + PartialOrd + Ord + Debug {
    const SIZE: Bytes;

    fn frames_needed(bytes: Bytes) -> Bytes {
        (bytes / Self::SIZE) + if bytes % Self::SIZE > 0 { 1 } else { 0 }
    }
}

macro frame_size($name: ident, $size: expr, $condition: meta) {
    #[$condition]
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
    pub enum $name {}

    #[$condition]
    impl FrameSize for $name {
        const SIZE: Bytes = $size;
    }
}

frame_size!(Size4KiB, kibibytes(4), cfg(target_arch = "x86_64"));
frame_size!(Size2MiB, mebibytes(2), cfg(target_arch = "x86_64"));
frame_size!(Size1GiB, gibibytes(1), cfg(target_arch = "x86_64"));

/// `FrameAllocator` is used to interact with a physical memory manager in a platform-independent way. Methods on
/// `FrameAllocator` take `&self` and so are expected to use interior-mutability through a type such as `Mutex` to
/// ensure safe access. This allows structures to store a reference to the allocator, and deallocate memory when
/// they're dropped.
///
/// A `FrameAllocator` is defined for a specific `FrameSize`, but multiple implementations of `FrameAllocator`
/// (each with a different frame size) can be used for allocators that aren't tied to a specific block size.
pub trait FrameAllocator<S>
where
    S: FrameSize,
{
    /// Allocate a `Frame`.
    ///
    /// By default, this calls `allocate_n(1)`, but can be overridden if an allocator can provide a
    /// more efficient method for allocating single frames.
    fn allocate(&self) -> Frame<S> {
        self.allocate_n(1).start
    }

    /// Allocate `n` contiguous `Frame`s.
    fn allocate_n(&self, n: usize) -> Range<Frame<S>>;

    /// Free `n` frames that were previously allocated by this allocator.
    fn free_n(&self, start: Frame<S>, n: usize);
}
