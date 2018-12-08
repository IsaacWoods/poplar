//! This module contains types for representing raw page tables in a type-safe way. We also don't
//! use the correct terminology for the levels of page tables (as they're confusing in the Intel
//! manual etc.) and so call them P4, P3, P2 and P1 respectively.

use super::entry::{Entry, EntryFlags};
use super::FrameAllocator;
use crate::memory::kernel_map::P4_TABLE_ADDRESS;
use crate::memory::VirtualAddress;
use core::marker::PhantomData;
use core::ops::{Index, IndexMut};

/// This points to the **currently installed** P4, **if** it is correctly recursively mapped. This
/// **does not** hold in the bootloader before we install our own tables.
pub(super) const P4: *mut Table<Level4, RecursiveMapping> = P4_TABLE_ADDRESS.mut_ptr();

pub enum Level4 {}
pub enum Level3 {}
pub enum Level2 {}
pub enum Level1 {}

pub trait TableLevel {}

impl TableLevel for Level4 {}
impl TableLevel for Level3 {}
impl TableLevel for Level2 {}
impl TableLevel for Level1 {}

/// Tables of levels that implement `HierarchicalLevel` are page tables whose entries are other
/// tables, as opposed to actual frames (like in P1s). This makes accessing the next level
/// type-safe, as the `next_table` methods are only implemented for tables that have child tables.
pub trait HierarchicalLevel: TableLevel {
    type NextLevel: TableLevel;
}

impl HierarchicalLevel for Level4 {
    type NextLevel = Level3;
}
impl HierarchicalLevel for Level3 {
    type NextLevel = Level2;
}
impl HierarchicalLevel for Level2 {
    type NextLevel = Level1;
}

/// This is a marker type that specifies that we are in an environment with an identity-mapped
/// virtual address space, and so we are able to access the page tables through their physical
/// addresses directly. This is true in the bootloader, as UEFI passes control to our bootloader
/// with the entire physical address space identity-mapped into the virtual address space.
///
/// This has to exist because we need to set up a set of page tables with a recursive mapping, but
/// we need a way to modify the page tables without a valid recursive mapping. This is made a lot
/// easier by the identity mapping that UEFI gives us anyway, so we use it to our advantage.
pub enum IdentityMapping {}

/// This is a marker type that specifies that we are in an environment where the active P4 page table
/// should always have a recursive entry - an entry that contains the physical address of the P4
/// itself. This allows us to access the backing memory of the page tables through special virtual
/// addresses that "loop" through the recursive P4 entry to access every level of the page tables.
///
/// This mapping is used by the kernel, and should be safe from the point the initial kernel page
/// tables are installed in the bootloader.
pub enum RecursiveMapping {}

/// This trait specifies how we should access and modify a set of page tables, allowing us to use
/// the same data structures in the bootloader and kernel. This is implemented by `IdentityMapping`
/// and `RecursiveMapping`.
pub trait TableMapping {
    fn next_table_address<L>(table: &Table<L, Self>, index: u16) -> Option<VirtualAddress>
    where
        L: HierarchicalLevel,
        Self: Sized;
}

impl TableMapping for IdentityMapping {
    fn next_table_address<L>(table: &Table<L, Self>, index: u16) -> Option<VirtualAddress>
    where
        L: HierarchicalLevel,
        Self: Sized,
    {
        /*
         * With an identity mapping, the virtual address of the page table will be the same as
         * the physical address of the frame that contains it, so we just return that.
         *
         * We don't need to check the entry's `PRESENT` flag because `pointed_frame` already does
         * it.
         */
        table[index]
            .pointed_frame()
            .and_then(|frame| VirtualAddress::new(u64::from(frame.start_address())))
    }
}

impl TableMapping for RecursiveMapping {
    fn next_table_address<L>(table: &Table<L, Self>, index: u16) -> Option<VirtualAddress>
    where
        L: HierarchicalLevel,
        Self: Sized,
    {
        let entry_flags = table[index].flags();

        if entry_flags.contains(EntryFlags::PRESENT) && !entry_flags.contains(EntryFlags::HUGE_PAGE)
        {
            /*
             * We can calculate the next table's address by going through one more layer of the
             * recursive mapping.
             *
             * XXX: This can make the address non-canonical when we shift the old address up into
             * the sign-extension bits, so we make sure to re-canonicalise it again.
             */
            let table_address = table as *const _ as u64;
            Some(VirtualAddress::new_canonicalise(
                (table_address << 9) | u64::from(index) << 12,
            ))
        } else {
            None
        }
    }
}

/// All page tables has 512 entries.
const ENTRY_COUNT: usize = 512;

/// Represents a page table, with 512 entries which are either child tables (in P4s, P3s and P2s)
/// or physical frames (in P1s). Every page table is exactly a page in length.
#[repr(transparent)]
pub struct Table<L: TableLevel, M: TableMapping> {
    entries: [Entry; ENTRY_COUNT],
    _level: PhantomData<L>,
    _mapping: PhantomData<M>,
}

impl<L, M> Table<L, M>
where
    L: TableLevel,
    M: TableMapping,
{
    pub fn zero(&mut self) {
        for entry in self.entries.iter_mut() {
            entry.set_unused();
        }
    }
}

/*
 * These methods are only implemented on tables with child tables.
 */
impl<L, M> Table<L, M>
where
    L: HierarchicalLevel,
    M: TableMapping,
{
    pub fn next_table(&self, index: u16) -> Option<&Table<L::NextLevel, M>> {
        M::next_table_address(&self, index).map(|address| unsafe { &*address.ptr() })
    }

    pub fn next_table_mut(&mut self, index: u16) -> Option<&mut Table<L::NextLevel, M>> {
        M::next_table_address(&self, index).map(|address| unsafe { &mut *address.mut_ptr() })
    }

    pub fn next_table_create<A>(
        &mut self,
        index: u16,
        user_accessible: bool,
        allocator: &A,
    ) -> &mut Table<L::NextLevel, M>
    where
        A: FrameAllocator,
    {
        if self.next_table(index).is_none() {
            assert!(
                !self.entries[index as usize]
                    .flags()
                    .contains(EntryFlags::HUGE_PAGE),
                "mapping code does not support huge pages"
            );

            self.entries[index as usize].set(
                allocator.allocate().unwrap(),
                EntryFlags::default()
                    | EntryFlags::WRITABLE
                    | if user_accessible {
                        EntryFlags::USER_ACCESSIBLE
                    } else {
                        EntryFlags::empty()
                    },
            );

            self.next_table_mut(index).unwrap().zero();
        }

        self.next_table_mut(index).unwrap()
    }
}

impl<L, M> Index<u16> for Table<L, M>
where
    L: TableLevel,
    M: TableMapping,
{
    type Output = Entry;

    fn index(&self, index: u16) -> &Entry {
        &self.entries[index as usize]
    }
}

impl<L, M> IndexMut<u16> for Table<L, M>
where
    L: TableLevel,
    M: TableMapping,
{
    fn index_mut(&mut self, index: u16) -> &mut Entry {
        &mut self.entries[index as usize]
    }
}
