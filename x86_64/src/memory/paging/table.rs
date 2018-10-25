//! This module contains types for representing raw page tables in a type-safe way. We also don't
//! use the correct terminology for the levels of page tables (as they're confusing in the Intel
//! manual etc.) and so call them P4, P3, P2 and P1 respectively.

use super::entry::{Entry, EntryFlags};
use super::FrameAllocator;
use core::marker::PhantomData;
use core::ops::{Index, IndexMut};
use crate::memory::kernel_map::P4_TABLE_ADDRESS;
use crate::memory::VirtualAddress;

/// This points to the **currently installed** P4, **if** it is correctly recursively mapped. This
/// **does not** hold in the bootloader before we install our own tables.
pub const P4: *mut Table<Level4> = P4_TABLE_ADDRESS.mut_ptr();

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

/// All page tables has 512 entries.
const ENTRY_COUNT: usize = 512;

/// Represents a page table, with 512 entries which are either child tables (in P4s, P3s and P2s)
/// or physical frames (in P1s). Every page table is exactly a page in length.
#[repr(transparent)]
pub struct Table<L: TableLevel> {
    entries: [Entry; ENTRY_COUNT],
    level: PhantomData<L>,
}

impl<L> Table<L>
where
    L: TableLevel,
{
    pub fn zero(&mut self) {
        for entry in self.entries.iter_mut() {
            entry.set_unused();
        }
    }
}

impl<L> Table<L>
where
    L: HierarchicalLevel,
{
    fn next_table_address(&self, index: u16) -> Option<VirtualAddress> {
        let entry_flags = self[index].flags();

        if entry_flags.contains(EntryFlags::PRESENT) && !entry_flags.contains(EntryFlags::HUGE_PAGE)
        {
            /*
             * We can calculate the next table's address by going through one more layer of the
             * recursive mapping.
             *
             * XXX: This can make the address non-canonical, as we shift the old table address up
             *      into the old sign-extension, so we make sure to canonicalise it again.
             */
            let table_address = self as *const _ as u64;
            Some(VirtualAddress::new_canonicalise(
                (table_address << 9) | (u64::from(index) << 12),
            ))
        } else {
            None
        }
    }

    pub fn next_table(&self, index: u16) -> Option<&Table<L::NextLevel>> {
        self.next_table_address(index)
            .map(|address| unsafe { &*address.ptr() })
    }

    pub fn next_table_mut(&mut self, index: u16) -> Option<&mut Table<L::NextLevel>> {
        self.next_table_address(index)
            .map(|address| unsafe { &mut *address.mut_ptr() })
    }

    pub fn next_table_create<A>(
        &mut self,
        index: u16,
        user_accessible: bool,
        allocator: &mut A,
    ) -> &mut Table<L::NextLevel>
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
                EntryFlags::default() | EntryFlags::WRITABLE | if user_accessible {
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

impl<L> Index<u16> for Table<L>
where
    L: TableLevel,
{
    type Output = Entry;

    fn index(&self, index: u16) -> &Entry {
        &self.entries[index as usize]
    }
}

impl<L> IndexMut<u16> for Table<L>
where
    L: TableLevel,
{
    fn index_mut(&mut self, index: u16) -> &mut Entry {
        &mut self.entries[index as usize]
    }
}
