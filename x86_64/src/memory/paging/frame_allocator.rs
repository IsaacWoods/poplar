use super::Frame;
use core::ops::Range;

/// `FrameAllocator` represents the `x86_64` crate's interface with the physical memory manager,
/// allowing it to remain independent of the actual method used to track allocated frames. This
/// allows us to use the crate from both the bootloader, where physical memory is managed by the
/// UEFI's boot services, and in the kernel, where we manage it manually.
///
/// Methods on `FrameAllocator` take `&self` and are expected to use interior mutability through
/// types such as a `Mutex`. This allows structures to store a shared reference to the allocator,
/// and deallocate their owned physical memory when they're dropped.
pub trait FrameAllocator {
    /// Allocate a `Frame`.
    ///
    /// By default, this calls `allocate_n(1)`, but can be overridden if an allocator can provide a
    /// more efficient method for allocating single frames.
    fn allocate(&self) -> Frame {
        self.allocate_n(1).start
    }

    /// Allocate `n` contiguous `Frame`s.
    fn allocate_n(&self, n: usize) -> Range<Frame>;

    /// Free `n` frames that were previously allocated by this `FrameAllocator`.
    fn free_n(&self, start: Frame, n: usize);
}
