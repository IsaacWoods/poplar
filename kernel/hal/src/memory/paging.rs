use super::{Frame, FrameAllocator, FrameSize, Page, PhysicalAddress, VirtualAddress};
use core::ops::Range;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Flags {
    pub writable: bool,
    pub executable: bool,
    pub user_accessible: bool,
    pub cached: bool,
}

impl Default for Flags {
    fn default() -> Self {
        Flags { writable: false, executable: false, user_accessible: false, cached: true }
    }
}

#[derive(Debug)]
pub enum PagingError {
    /// The virtual memory that is being mapped is already mapped to another part of physical memory.
    AlreadyMapped,
}

/// A `PageTable` allows the manipulation of a set of page-tables.
// TODO: think about how we can do versatile unmapping (maybe return a `Map` type that is returned to unmap - this
// could store information needed to unmap an artitrarily-mapped area).
pub trait PageTable<TableSize>: Sized
where
    TableSize: FrameSize,
{
    /// Construct a new set of page tables that are suitable for an `AddressSpace` kernel object - one that can
    /// hold userspace tasks. This generally needs the kernel mapped into it somehow, so we pass in the kernel's
    /// set of page tables.
    fn new_for_address_space<A>(kernel_page_table: &Self, allocator: &A) -> Self
    where
        A: FrameAllocator<TableSize>;

    /// Install these page tables as the current set.
    fn switch_to(&self);

    /// Get the physical address that a given virtual address is mapped to, if it's mapped. Returns `None` if the
    /// address is not mapped into physical memory.
    fn translate(&self, address: VirtualAddress) -> Option<PhysicalAddress>;

    /// Map a `Page` to a `Frame` with the given flags.
    fn map<A, S>(
        &mut self,
        page: Page<S>,
        frame: Frame<S>,
        flags: Flags,
        allocator: &A,
    ) -> Result<(), PagingError>
    where
        A: FrameAllocator<TableSize>,
        S: FrameSize;

    /// Map each `Page` in a range to a corresponding `Frame` with the given flags.
    fn map_range<A, S>(
        &mut self,
        pages: Range<Page<S>>,
        frames: Range<Frame<S>>,
        flags: Flags,
        allocator: &A,
    ) -> Result<(), PagingError>
    where
        A: FrameAllocator<TableSize>,
        S: FrameSize,
    {
        for (page, frame) in pages.zip(frames) {
            self.map(page, frame, flags, allocator)?;
        }

        Ok(())
    }

    /// Map an area of `size` bytes starting at the given address pair with the given flags. Implementations are
    /// free to map this area however they desire, and may do so with a range of page sizes.
    fn map_area<A>(
        &mut self,
        virtual_start: VirtualAddress,
        physical_start: PhysicalAddress,
        size: usize,
        flags: Flags,
        allocator: &A,
    ) -> Result<(), PagingError>
    where
        A: FrameAllocator<TableSize>;

    fn unmap<S>(&mut self, page: Page<S>) -> Option<Frame<S>>
    where
        S: FrameSize;
}
