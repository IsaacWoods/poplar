use super::{Frame, FrameAllocator, FrameSize, Page, PhysicalAddress, VirtualAddress};
use core::ops::Range;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Flags {
    writable: bool,
    executable: bool,
    user_accessible: bool,
}

pub enum MapperError {
    NotMapped,
}

/// A `Mapper` allows the manipulation of a set of page-tables.
// TODO: think about how we can do versatile unmapping (maybe return a `Map` type that is returned to unmap - this
// could store information needed to unmap an artitrarily-mapped area).
pub trait Mapper<TableSize, TableAllocator>
where
    TableSize: FrameSize,
    TableAllocator: FrameAllocator<TableSize>,
{
    /// Get the physical address that a given virtual address is mapped to, if it's mapped.
    fn translate(&self, address: VirtualAddress) -> Result<PhysicalAddress, MapperError>;

    /// Map a `Page` to a `Frame` with the given flags.
    fn map<S>(
        &mut self,
        page: Page<S>,
        frame: Frame<S>,
        flags: Flags,
        allocator: TableAllocator,
    ) -> Result<(), MapperError>
    where
        S: FrameSize;

    /// Map each `Page` in a range to a corresponding `Frame` with the given flags.
    fn map_range<S>(
        &mut self,
        pages: Range<Page<S>>,
        frames: Range<Frame<S>>,
        flags: Flags,
        allocator: TableAllocator,
    ) -> Result<(), MapperError>
    where
        S: FrameSize,
    {
        for (page, frame) in pages.zip(frames) {
            self.map(page, frame, flags, allocator)?;
        }

        Ok(())
    }

    /// Map an area of `size` bytes starting at the given address pair with the given flags. Implementations are
    /// free to map this area however they desire, and may do so with a range of page sizes.
    fn map_area(
        &mut self,
        virtual_start: VirtualAddress,
        physical_start: PhysicalAddress,
        size: usize,
        flags: Flags,
        allocator: TableAllocator,
    ) -> Result<(), MapperError>;
}
