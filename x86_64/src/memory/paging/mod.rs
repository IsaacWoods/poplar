pub mod entry;
pub mod frame;
pub mod frame_allocator;
pub mod mapper;
pub mod page;
pub mod table;

pub use self::frame::Frame;
pub use self::frame_allocator::FrameAllocator;
pub use self::mapper::Mapper;
pub use self::page::Page;
pub use core::ops::{Deref, DerefMut};

pub const FRAME_SIZE: u64 = 0x1000;
pub const PAGE_SIZE: u64 = 0x1000;

use self::table::{IdentityMapping, RecursiveMapping, TableMapping};
use super::PhysicalAddress;
use core::marker::PhantomData;

/// Represents a set of page tables that are not currently mapped.
pub struct InactivePageTable<M>
where
    M: TableMapping,
{
    p4_frame: Frame,
    _mapping: PhantomData<M>,
}

impl<M> InactivePageTable<M>
where
    M: TableMapping,
{
    /// Create a new set of page-tables. `frame` must be an allocated, **zeroed** `Frame` of
    /// physical memory. We don't zero the memory here because to do that we need to map it into
    /// the active set of page tables, which aren't available when we first create an
    /// `InactivePageTable` in the bootloader.
    pub fn new(frame: Frame) -> InactivePageTable<M> {
        InactivePageTable {
            p4_frame: frame,
            _mapping: PhantomData,
        }
    }

    /// Switch to this set of page tables. This returns a tuple containing the new
    /// `ActivePageTable` (that this has become), and the previously-active set of tables as an
    /// `InactivePageTable`.
    ///
    /// Unsafe because you are required to specify the correct `TableMapping` for the currently
    /// installed set of page tables (the one that is returned as an `InactivePageTable<A>`), as
    /// this can't be type-checked.
    ///
    /// # Generic parameters
    /// The two generic parameters, `O` and `N` denote the mappings of the newly-inactive and the
    /// newly-active set of page tables respectively. For example, if you create, in an
    /// identity-mapped environment, an `InactivePageTable<IdentityMapping>` with a recursive
    /// mapping, `O` should be `IdentityMapping` and `N` should be `RecursiveMapping`.
    pub unsafe fn switch_to<O, N>(self) -> (ActivePageTable<N>, InactivePageTable<O>)
    where
        O: TableMapping,
        N: TableMapping,
    {
        // TODO
        unimplemented!();
    }
}

/// Represents the set of page tables that are currently being used. The recursive mapping will
/// point to the address of these tables, and so it's safe to create a `Mapper` for an
/// `ActivePageTable`.
pub struct ActivePageTable<M>
where
    M: 'static + TableMapping,
{
    mapper: Mapper<M>,
}

impl<M> Deref for ActivePageTable<M>
where
    M: 'static + TableMapping,
{
    type Target = Mapper<M>;

    fn deref(&self) -> &Self::Target {
        &self.mapper
    }
}

impl<M> DerefMut for ActivePageTable<M>
where
    M: 'static + TableMapping,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.mapper
    }
}

impl ActivePageTable<IdentityMapping> {
    /// Create an `ActivePageTable` to represent an active set of page tables that should be
    /// accessed directly using their physical addresses. This only works in an environment with an
    /// identity-mapped virtual address space (such as in the UEFI bootloader), and should be used
    /// before we have a set of page tables that are recursively mapped.
    pub unsafe fn new(p4_address: PhysicalAddress) -> ActivePageTable<IdentityMapping> {
        ActivePageTable {
            mapper: Mapper::<IdentityMapping>::new(p4_address),
        }
    }
}

impl ActivePageTable<RecursiveMapping> {
    /// Create an `ActivePageTable` to represent the currently-installed set of page tables. This
    /// is unsafe because it assumes a valid set of page tables exist and are pointed to by `CR3`,
    /// and that they are correctly recursively mapped.
    pub unsafe fn new() -> ActivePageTable<RecursiveMapping> {
        ActivePageTable {
            mapper: Mapper::<RecursiveMapping>::new(),
        }
    }

    /// Alter the mappings of a `InactivePageTable` by temporarily replacing the recursive entry
    /// address of the active tables with the physical address of the inactive table's P4.
    ///
    /// This calls the closure with a `Mapper` that targets the current set of active tables, but
    /// will actually modify the given `InactivePageTable`'s mappings. Because the inactive table
    /// isn't really mapped, you can't modify the *contents* of the mappings. To modify the
    /// physical memory, you will either need to switch to the `InactivePageTable`, or map it into
    /// the `ActivePageTable` temporarily.
    pub fn with<A, F>(
        &mut self,
        table: &mut InactivePageTable<RecursiveMapping>,
        frame_allocator: &A,
        f: F,
    ) where
        A: FrameAllocator,
        F: FnOnce(&mut Mapper<RecursiveMapping>, &A),
    {
        // TODO
        unimplemented!();
    }
}
