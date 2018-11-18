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
    /// Allocate a `Frame`. If there are no remaining free frames, the allocator is free to take
    /// measures to free physical memory, or to kernel panic. It is always safe to `unwrap` the
    /// return value, as the `Err` branch is diverging (marking the possibility of kernel panic).
    ///
    /// By default, this calls `allocate_n(1)`, but can be overridden if an allocator can provide a
    /// more efficient method for allocating single frames.
    fn allocate(&self) -> Result<Frame, !> {
        self.allocate_n(1).map(|range| range.start)
    }

    /// Allocate `n` contiguous `Frame`s, if possible.
    fn allocate_n(&self, n: u64) -> Result<Range<Frame>, !>;

    /// Free a previously-allocated frame, marking it available for allocation in the future.
    fn free(&self, frame: Frame);
}
