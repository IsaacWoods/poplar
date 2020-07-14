use super::{Frame, FrameAllocator, FrameSize, Page, PhysicalAddress, VirtualAddress};
use core::ops::{self, Range};

/// Defines the permissions for a region of memory. Used both for abstract regions of memory (e.g. entries in a
/// memory map) and as a architecture-common representation of paging structures.
///
/// The `Add` implementation "coalesces" two sets of `Flags`, giving a set of `Flags` that has the permissions of
/// both of the sets. For example, if one region is writable and the other is not, the coalesced flags will be
/// writable. By default, a region is considered to be cached, so coalesced flags will only be cached if both input
/// regions can safely be cached.
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

impl ops::Add for Flags {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        Flags {
            writable: self.writable || other.writable,
            executable: self.executable || other.executable,
            user_accessible: self.user_accessible || other.user_accessible,
            // If either of the regions should not be cached, we can't cache any of it
            cached: self.cached && other.cached,
        }
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
    /// Constructs a new set of page tables, but with the kernel mapped into it. This is generally useful for
    /// constructing page tables for userspace.
    fn new_with_kernel_mapped<A>(kernel_page_table: &Self, allocator: &A) -> Self
    where
        A: FrameAllocator<TableSize>;

    /// Install these page tables as the current set.
    fn switch_to(&self);

    /// Get the physical address that a given virtual address is mapped to, if it's mapped. Returns `None` if the
    /// address is not mapped into physical memory.
    fn translate(&self, address: VirtualAddress) -> Option<PhysicalAddress>;

    /// Map a `Page` to a `Frame` with the given flags.
    fn map<S, A>(
        &mut self,
        page: Page<S>,
        frame: Frame<S>,
        flags: Flags,
        allocator: &A,
    ) -> Result<(), PagingError>
    where
        S: FrameSize,
        A: FrameAllocator<TableSize>;

    /// Map each `Page` in a range to a corresponding `Frame` with the given flags.
    fn map_range<S, A>(
        &mut self,
        pages: Range<Page<S>>,
        frames: Range<Frame<S>>,
        flags: Flags,
        allocator: &A,
    ) -> Result<(), PagingError>
    where
        S: FrameSize,
        A: FrameAllocator<TableSize>,
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

/// Split an address range into three ranges - one that is aligned to the given `FrameSize`, and a range either
/// side of it that is not. This is useful for splitting an arbitrary region into a part that can be mapped with
/// larger frames, and parts that can not.
pub fn split_region<S>(
    range: Range<VirtualAddress>,
) -> (Option<Range<VirtualAddress>>, Option<Range<VirtualAddress>>, Option<Range<VirtualAddress>>)
where
    S: FrameSize,
{
    let aligned_start = range.start.align_up(S::SIZE);
    let aligned_end = range.end.align_down(S::SIZE);

    if aligned_start <= range.end && aligned_end >= range.start {
        (
            if aligned_start == range.start { None } else { Some(range.start..aligned_start) },
            if aligned_start == aligned_end { None } else { Some(aligned_start..aligned_end) },
            if aligned_end == range.end { None } else { Some(aligned_end..range.end) },
        )
    } else {
        (Some(range.start..range.end), None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{Size1GiB, Size2MiB, Size4KiB};

    #[test]
    fn test_flag_coalescing() {
        assert_eq!(Flags::default() + Flags::default(), Flags::default());
        assert_eq!(
            Flags::default() + Flags { writable: false, executable: true, user_accessible: true, cached: true },
            Flags { writable: false, executable: true, user_accessible: true, cached: true }
        );
        assert_eq!(
            Flags::default() + Flags { writable: true, executable: true, user_accessible: true, cached: true },
            Flags { writable: true, executable: true, user_accessible: true, cached: true }
        );
        assert_eq!(
            Flags::default() + Flags { cached: false, ..Default::default() },
            Flags { cached: false, ..Default::default() }
        );
        assert_eq!(
            Flags { cached: false, ..Default::default() } + Flags { cached: false, ..Default::default() },
            Flags { cached: false, ..Default::default() }
        );
    }

    #[test]
    fn test_split_region() {
        /*
         * Test "splitting" a bunch of 4KiB pages.
         */
        assert_eq!(
            split_region::<Size4KiB>(VirtualAddress::new(0x6000)..VirtualAddress::new(0xa4000)),
            (None, Some(VirtualAddress::new(0x6000)..VirtualAddress::new(0xa4000)), None)
        );

        /*
         * Split a large region into a 2MiB-aligned section with a tail.
         */
        assert_eq!(
            split_region::<Size2MiB>(VirtualAddress::new(0x0)..VirtualAddress::new(0x400000 + 0xf000)),
            (
                None,
                Some(VirtualAddress::new(0x0)..VirtualAddress::new(0x400000)),
                Some(VirtualAddress::new(0x400000)..VirtualAddress::new(0x40f000))
            )
        );

        /*
         * Split a large region into a 2MiB-aligned section with both a head and a tail.
         */
        assert_eq!(
            split_region::<Size2MiB>(
                VirtualAddress::new(0x200000 - 0x4000)..VirtualAddress::new(0x800000 + 0x13000)
            ),
            (
                Some(VirtualAddress::new(0x1fc000)..VirtualAddress::new(0x200000)),
                Some(VirtualAddress::new(0x200000)..VirtualAddress::new(0x800000)),
                Some(VirtualAddress::new(0x800000)..VirtualAddress::new(0x813000))
            )
        );

        /*
         * Split a region of more than 1GiB with both a head and tail.
         */
        assert_eq!(
            split_region::<Size1GiB>(
                VirtualAddress::new(0x40000000 - 0x435000)..VirtualAddress::new(0x100000000 + 0x6000)
            ),
            (
                Some(VirtualAddress::new(0x3fbcb000)..VirtualAddress::new(0x40000000)),
                Some(VirtualAddress::new(0x40000000)..VirtualAddress::new(0x100000000)),
                Some(VirtualAddress::new(0x100000000)..VirtualAddress::new(0x100006000))
            )
        );
    }
}
