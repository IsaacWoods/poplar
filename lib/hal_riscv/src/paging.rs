/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

use crate::hw::csr::Satp;
use bit_field::BitField;
use bitflags::bitflags;
use core::{
    arch::asm,
    cmp,
    fmt,
    fmt::Debug,
    marker::PhantomData,
    ops::{Index, IndexMut},
};
use hal::memory::{
    Flags,
    Frame,
    FrameAllocator,
    FrameSize,
    PAddr,
    Page,
    PageTable,
    PagingError,
    Size1GiB,
    Size2MiB,
    Size4KiB,
    VAddr,
};

bitflags! {
    pub struct EntryFlags: u64 {
        const VALID             = 1 << 0;
        const READABLE          = 1 << 1;
        const WRITABLE          = 1 << 2;
        const EXECUTABLE        = 1 << 3;
        const USER_ACCESSIBLE   = 1 << 4;
        const GLOBAL            = 1 << 5;
        const ACCESSED          = 1 << 6;
        const DIRTY             = 1 << 7;
    }
}

impl From<Flags> for EntryFlags {
    fn from(flags: Flags) -> Self {
        // TODO: should we do anything with `flags.cached` here?
        // TODO: should we expose the readable flag in `hal`? Bc x64 can't choose? I think so to expose ability to have executable-only pages?
        EntryFlags::VALID
            | if flags.writable { EntryFlags::READABLE | EntryFlags::WRITABLE } else { EntryFlags::READABLE }
            | if flags.executable { EntryFlags::EXECUTABLE } else { EntryFlags::empty() }
            | if flags.user_accessible { EntryFlags::USER_ACCESSIBLE } else { EntryFlags::empty() }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Entry(u64);

impl Entry {
    pub fn unused() -> Self {
        Self(0)
    }

    pub fn flags(&self) -> EntryFlags {
        EntryFlags::from_bits_truncate(self.0)
    }

    pub fn is_valid(&self) -> bool {
        self.flags().contains(EntryFlags::VALID)
    }

    /// Returns `true` if this is a leaf entry - that is, this entry specifies a mapping to a frame of the same
    /// size, rather than the next level of the page table.
    pub fn is_leaf(&self) -> bool {
        let flags = self.flags();
        flags.contains(EntryFlags::VALID)
            && flags.intersects(EntryFlags::READABLE | EntryFlags::WRITABLE | EntryFlags::EXECUTABLE)
    }

    pub fn address(&self) -> Option<PAddr> {
        if self.flags().contains(EntryFlags::VALID) {
            Some(PAddr::new((self.0.get_bits(10..54) as usize) << 12).unwrap())
        } else {
            None
        }
    }

    pub fn set(&mut self, entry: Option<(PAddr, EntryFlags)>, is_leaf: bool) {
        self.0 = match entry {
            Some((address, flags)) => {
                let flags = flags
                    | EntryFlags::VALID
                    | if is_leaf { EntryFlags::ACCESSED | EntryFlags::DIRTY } else { EntryFlags::empty() };
                (usize::from(address) as u64 >> 2) | flags.bits()
            }
            None => 0,
        };
    }
}

impl Debug for Entry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_valid() {
            f.debug_tuple("Entry").field(&self.address().unwrap()).field(&self.flags()).finish()
        } else {
            write!(f, "Not Present")
        }
    }
}

// TODO: lots of this stuff has been duplicated from `hal_x86_64`; abstract into `hal`?
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

const ENTRY_COUNT: usize = 512;

#[repr(C, align(4096))]
pub struct Table<L>
where
    L: TableLevel,
{
    entries: [Entry; ENTRY_COUNT],
    _phantom: PhantomData<L>,
}

impl<L> Table<L>
where
    L: TableLevel,
{
    pub fn new(&mut self) -> Table<L> {
        Table { entries: [Entry::unused(); ENTRY_COUNT], _phantom: PhantomData }
    }
}

impl<L> Index<usize> for Table<L>
where
    L: TableLevel,
{
    type Output = Entry;

    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[index]
    }
}

impl<L> IndexMut<usize> for Table<L>
where
    L: TableLevel,
{
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.entries[index]
    }
}

impl<L> Table<L>
where
    L: TableLevel,
{
    pub fn zero(&mut self) {
        for entry in self.entries.iter_mut() {
            entry.set(None, false);
        }
    }
}

impl<L> Table<L>
where
    L: HierarchicalLevel,
{
    /// Get a reference to the table at the given `index`, assuming the entirity of
    /// the physical address space is mapped from `physical_base`.
    pub fn next_table(&self, index: usize, physical_base: VAddr) -> Option<&Table<L::NextLevel>> {
        self[index]
            .address()
            .map(|physical_address| physical_base + usize::from(physical_address))
            .map(|virtual_address| unsafe { &*(virtual_address.ptr()) })
    }

    /// Get a mutable reference to the table at the given `index`, assuming the entirity of
    /// the physical address space is mapped from `physical_base`.
    pub fn next_table_mut(&mut self, index: usize, physical_base: VAddr) -> Option<&mut Table<L::NextLevel>> {
        self[index]
            .address()
            .map(|physical_address| physical_base + usize::from(physical_address))
            .map(|virtual_address| unsafe { &mut *(virtual_address.mut_ptr()) })
    }

    pub fn next_table_create<A>(
        &mut self,
        index: usize,
        allocator: &A,
        physical_base: VAddr,
    ) -> Result<&mut Table<L::NextLevel>, PagingError>
    where
        A: FrameAllocator<Size4KiB>,
    {
        if self.next_table(index, physical_base).is_none() {
            /*
             * This entry is empty, so we create a new page table, zero it, and return that.
             */
            self.entries[index].set(Some((allocator.allocate().start, EntryFlags::VALID)), false);
            let table = self.next_table_mut(index, physical_base).unwrap();
            table.zero();
            Ok(table)
        } else {
            /*
             * This entry already exists, so we don't need to create another one. However, we do need to detect a
             * special case here: when we're seeing if we need to create a parent table in order to map into lower
             * tables (e.g. creating a P2 to create a P1 for 4KiB mappings), there might already be a huge page
             * mapped into the parent table. If this occurs, we error because the whole region has already been
             * mapped.
             */
            if self[index].is_leaf() {
                return Err(PagingError::AlreadyMapped);
            }

            Ok(self.next_table_mut(index, physical_base).unwrap())
        }
    }
}

// TODO: make generic over which level of table is the top
pub struct PageTableImpl<T: HierarchicalLevel> {
    /// The frame that holds the top-level table.
    frame: Frame,
    /// The virtual address at which physical memory is mapped in the environment that these page
    /// tables are being constructed in. This is **not** a property of the set of page tables being
    /// mapped, but of the context the tables are being modified from.
    physical_base: VAddr,
    _phantom: PhantomData<T>,
}

impl<T> PageTableImpl<T>
where
    T: HierarchicalLevel,
{
    pub fn new(frame: Frame, physical_base: VAddr) -> PageTableImpl<T> {
        let mut table = PageTableImpl { frame, physical_base, _phantom: PhantomData };
        table.top_mut().zero();
        table
    }

    /// Create a `PageTableImpl` from a `Frame` that already contains a top-level table. This is
    /// very unsafe because it assumes that the frame contains a valid page table, and that no
    /// other `PageTableImpl`s currently exist that use this same backing frame (as calling
    /// `mapper` on both could lead to two mutable references aliasing the same data to exist,
    /// which is UB).
    pub unsafe fn from_frame(frame: Frame, physical_base: VAddr) -> PageTableImpl<T> {
        PageTableImpl { frame, physical_base, _phantom: PhantomData }
    }

    pub fn top(&self) -> &Table<T> {
        unsafe { &*((self.physical_base + usize::from(self.frame.start)).ptr()) }
    }

    pub fn top_mut(&mut self) -> &mut Table<T> {
        unsafe { &mut *((self.physical_base + usize::from(self.frame.start)).mut_ptr()) }
    }
}

/*
 * Implementation for `Sv48` systems, which support four levels of tables.
 */
impl PageTableImpl<Level4> {
    pub fn satp(&self) -> Satp {
        Satp::Sv48 { asid: 0, root: self.frame.start }
    }
}

impl fmt::Debug for PageTableImpl<Level4> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "PageTable {{")?;
        let p4 = self.top();
        for i in 0..512 {
            if p4[i].is_valid() {
                writeln!(f, "    P4 entry {}({:#x}): {:?}", i, VAddr::from_indices(i, 0, 0, 0), p4[i])?;
                if p4[i].is_leaf() {
                    continue;
                }
                let p3 = p4.next_table(i, self.physical_base).unwrap();
                for j in 0..512 {
                    if p3[j].is_valid() {
                        writeln!(
                            f,
                            "        P3 entry {}({:#x}): {:?}",
                            j,
                            VAddr::from_indices(i, j, 0, 0),
                            p3[j]
                        )?;
                        if p3[j].is_leaf() {
                            continue;
                        }
                        let p2 = p3.next_table(j, self.physical_base).unwrap();
                        for k in 0..512 {
                            if p2[k].is_valid() {
                                writeln!(
                                    f,
                                    "            P2 entry {}({:#x}): {:?}",
                                    k,
                                    VAddr::from_indices(i, j, k, 0),
                                    p2[k]
                                )?;
                                if p2[k].is_leaf() {
                                    continue;
                                }
                                let p1 = p2.next_table(k, self.physical_base).unwrap();
                                for m in 0..512 {
                                    if p1[m].is_valid() {
                                        writeln!(
                                            f,
                                            "                P1 entry {}({:#x}): {:?}",
                                            m,
                                            VAddr::from_indices(i, j, k, m),
                                            p1[m]
                                        )?;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        writeln!(f, "}}")?;
        Ok(())
    }
}

impl PageTable<Size4KiB> for PageTableImpl<Level4> {
    fn new_with_kernel_mapped<A>(kernel_page_table: &Self, allocator: &A) -> Self
    where
        A: FrameAllocator<Size4KiB>,
    {
        let mut page_table =
            PageTableImpl::new(allocator.allocate(), crate::platform::kernel_map::PHYSICAL_MAP_BASE);

        /*
         * Install the address of the kernel's P3 in every address space, so that the kernel is always mapped.
         * It's safe to unwrap the kernel P3 address, as we wouldn't be able to fetch these instructions
         * if it wasn't there.
         */
        let kernel_p3_address =
            kernel_page_table.top()[crate::platform::kernel_map::KERNEL_TABLE_ENTRY].address().unwrap();
        page_table.top_mut()[crate::platform::kernel_map::KERNEL_TABLE_ENTRY]
            .set(Some((kernel_p3_address, EntryFlags::empty())), false);

        page_table
    }

    unsafe fn switch_to(&self) {
        unsafe { self.satp().write() }
    }

    fn translate(&self, address: VAddr) -> Option<PAddr> {
        // TODO: handle huge pages at the P3 level as well

        let p2 = self
            .top()
            .next_table(address.p4_index(), self.physical_base)
            .and_then(|p3| p3.next_table(address.p3_index(), self.physical_base))?;

        let p2_entry = p2[address.p2_index()];
        if p2_entry.is_leaf() {
            return Some(p2_entry.address()? + (usize::from(address) % Size2MiB::SIZE));
        }

        let p1 = p2.next_table(address.p2_index(), self.physical_base)?;
        Some(p1[address.p1_index()].address()? + (usize::from(address) % Size4KiB::SIZE))
    }

    fn map<S, A>(&mut self, page: Page<S>, frame: Frame<S>, flags: Flags, allocator: &A) -> Result<(), PagingError>
    where
        S: FrameSize,
        A: FrameAllocator<Size4KiB>,
    {
        let physical_base = self.physical_base;

        if S::SIZE == Size4KiB::SIZE {
            let p1 = self
                .top_mut()
                .next_table_create(page.start.p4_index(), allocator, physical_base)?
                .next_table_create(page.start.p3_index(), allocator, physical_base)?
                .next_table_create(page.start.p2_index(), allocator, physical_base)?;

            if p1[page.start.p1_index()].is_valid() {
                return Err(PagingError::AlreadyMapped);
            }

            p1[page.start.p1_index()].set(Some((frame.start, EntryFlags::from(flags))), true);
        } else if S::SIZE == Size2MiB::SIZE {
            let p2 = self
                .top_mut()
                .next_table_create(page.start.p4_index(), allocator, physical_base)?
                .next_table_create(page.start.p3_index(), allocator, physical_base)?;

            if p2[page.start.p2_index()].is_valid() {
                return Err(PagingError::AlreadyMapped);
            }

            p2[page.start.p2_index()].set(Some((frame.start, EntryFlags::from(flags))), true);
        } else {
            assert_eq!(S::SIZE, Size1GiB::SIZE);

            let p3 = self.top_mut().next_table_create(page.start.p4_index(), allocator, physical_base)?;

            if p3[page.start.p3_index()].is_valid() {
                return Err(PagingError::AlreadyMapped);
            }

            p3[page.start.p3_index()].set(Some((frame.start, EntryFlags::from(flags))), true);
        }

        // TODO: replace this with a returned 'token' or whatever to batch changes before a flush if possible
        sfence_vma(None, Some(page.start));
        Ok(())
    }

    fn map_area<A>(
        &mut self,
        virtual_start: VAddr,
        physical_start: PAddr,
        size: usize,
        flags: Flags,
        allocator: &A,
    ) -> Result<(), PagingError>
    where
        A: FrameAllocator<Size4KiB>,
    {
        use mulch::math::{abs_difference, align_down};

        assert!(virtual_start.is_aligned(Size4KiB::SIZE));
        assert!(physical_start.is_aligned(Size4KiB::SIZE));
        assert!(size % Size4KiB::SIZE == 0);

        /*
         * If the area is smaller than a single 2MiB page, or if the virtual and physical starts are "out of
         * phase" such that we'll never be able to use larger pages, just use 4KiB pages.
         */
        let align_mismatch =
            abs_difference(usize::from(physical_start), usize::from(virtual_start)) % Size2MiB::SIZE != 0;
        if size < Size2MiB::SIZE || align_mismatch {
            let pages = Page::starts_with(virtual_start)..Page::starts_with(virtual_start + size);
            let frames = Frame::starts_with(physical_start)..Frame::starts_with(physical_start + size);
            return self.map_range::<Size4KiB, A>(pages, frames, flags, allocator);
        }

        let mut cursor = virtual_start;
        let virtual_end: VAddr = virtual_start + size;

        while cursor < virtual_end {
            let cursor_physical =
                PAddr::new(usize::from(physical_start) + usize::from(cursor) - usize::from(virtual_start))
                    .unwrap();
            let bytes_left = usize::from(virtual_end) - usize::from(cursor);

            if cursor.is_aligned(Size1GiB::SIZE)
                && cursor_physical.is_aligned(Size1GiB::SIZE)
                && bytes_left >= Size1GiB::SIZE
            {
                /*
                 * We can fit at least 1GiB page in, and both virtual and physical cursors have the correct
                 * alignment. Map as much as we can with 1GiB pages.
                 */
                let bytes_to_map = align_down(bytes_left, Size1GiB::SIZE);
                let pages = Page::starts_with(cursor)..Page::starts_with(cursor + bytes_to_map);
                let frames =
                    Frame::starts_with(cursor_physical)..Frame::starts_with(cursor_physical + bytes_to_map);
                self.map_range::<Size1GiB, A>(pages, frames, flags, allocator)?;
                cursor += bytes_to_map;
            } else if cursor.is_aligned(Size2MiB::SIZE)
                && cursor_physical.is_aligned(Size2MiB::SIZE)
                && bytes_left >= Size2MiB::SIZE
            {
                /*
                 * We couldn't use a 1GiB page, but we can use 2MiB pages! Map as much as we can.
                 *
                 * TODO: we could do a similar thing to below to check if we can use 1GiB pages further in, but
                 * it's probably unlikely enough that it's not really worth it.
                 */
                let bytes_to_map = align_down(bytes_left, Size2MiB::SIZE);
                let pages = Page::starts_with(cursor)..Page::starts_with(cursor + bytes_to_map);
                let frames =
                    Frame::starts_with(cursor_physical)..Frame::starts_with(cursor_physical + bytes_to_map);
                self.map_range::<Size2MiB, A>(pages, frames, flags, allocator)?;
                cursor += bytes_to_map;
            } else {
                /*
                 * We can't use any larger pages, but we might be able to further in, if the data becomes more
                 * aligned. If the next 2MiB-aligned address is still inside the range, stop there to have another
                 * go.
                 * NOTE: `cursor` might be 2MiB-aligned at this location, so we start from the next address so we don't get stuck here.
                 */
                let next_boundary = (cursor + 1).align_up(Size2MiB::SIZE);
                // Make sure not to go past the end of the region
                let bytes_to_map = cmp::min(
                    usize::from(next_boundary) - usize::from(cursor),
                    usize::from(virtual_end) - usize::from(cursor),
                );
                let pages = Page::starts_with(cursor)..Page::starts_with(cursor + bytes_to_map);
                let frames =
                    Frame::starts_with(cursor_physical)..Frame::starts_with(cursor_physical + bytes_to_map);
                self.map_range::<Size4KiB, A>(pages, frames, flags, allocator)?;
                cursor += bytes_to_map;
            }
        }

        assert_eq!(cursor, virtual_end);
        Ok(())
    }

    fn unmap<S>(&mut self, page: Page<S>) -> Option<Frame<S>>
    where
        S: FrameSize,
    {
        let physical_base = self.physical_base;

        match S::SIZE {
            Size4KiB::SIZE => {
                let p1 = self
                    .top_mut()
                    .next_table_mut(page.start.p4_index(), physical_base)?
                    .next_table_mut(page.start.p3_index(), physical_base)?
                    .next_table_mut(page.start.p2_index(), physical_base)?;
                let frame = Frame::starts_with(p1[page.start.p1_index()].address()?);
                p1[page.start.p1_index()].set(None, true);
                sfence_vma(None, Some(page.start));

                Some(frame)
            }
            Size2MiB::SIZE => unimplemented!(),
            Size1GiB::SIZE => unimplemented!(),

            _ => panic!("Unimplemented page size!"),
        }
    }
}

/*
 * Implementation for `Sv39` systems, which support three levels of tables.
 */
impl PageTableImpl<Level3> {
    pub fn satp(&self) -> Satp {
        Satp::Sv39 { asid: 0, root: self.frame.start }
    }
}

impl fmt::Debug for PageTableImpl<Level3> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "PageTable {{")?;
        let p3 = self.top();
        for i in 0..512 {
            if p3[i].is_valid() {
                writeln!(f, "    P3 entry {}: {:?}", i, p3[i])?;
                if p3[i].is_leaf() {
                    continue;
                }
                let p2 = p3.next_table(i, self.physical_base).unwrap();
                for j in 0..512 {
                    if p2[j].is_valid() {
                        writeln!(f, "        P2 entry {}: {:?}", j, p2[j])?;
                        if p2[j].is_leaf() {
                            continue;
                        }
                        let p1 = p2.next_table(j, self.physical_base).unwrap();
                        for k in 0..512 {
                            if p1[k].is_valid() {
                                writeln!(f, "            P1 entry {}: {:?}", k, p1[k])?;
                            }
                        }
                    }
                }
            }
        }
        writeln!(f, "}}")?;
        Ok(())
    }
}

impl PageTable<Size4KiB> for PageTableImpl<Level3> {
    fn new_with_kernel_mapped<A>(kernel_page_table: &Self, allocator: &A) -> Self
    where
        A: FrameAllocator<Size4KiB>,
    {
        let mut page_table =
            PageTableImpl::new(allocator.allocate(), crate::platform::kernel_map::PHYSICAL_MAP_BASE);

        /*
         * For three-level paging schemes, the entire upper half of the address space belongs to
         * kernel-space. We need to walk the upper half of the kernel's P3 and copy it across.
         * TODO: This could be problematic because the kernel could realistically need to map new
         * top-level entries during the runtime, in which case tasks' page tables would need
         * updating. Probably worth thinking about at some point...
         * TODO: I wonder if we could pre-allocate these so they all get copied across properly?
         */
        for i in (ENTRY_COUNT / 2)..ENTRY_COUNT {
            page_table.top_mut()[i] = kernel_page_table.top()[i];
        }

        page_table
    }

    unsafe fn switch_to(&self) {
        unsafe { self.satp().write() }
    }

    fn translate(&self, address: VAddr) -> Option<PAddr> {
        // TODO: handle huge pages at the P3 level as well

        let p2 = self.top().next_table(address.p3_index(), self.physical_base)?;

        let p2_entry = p2[address.p2_index()];
        if p2_entry.is_leaf() {
            return Some(p2_entry.address()? + (usize::from(address) % Size2MiB::SIZE));
        }

        let p1 = p2.next_table(address.p2_index(), self.physical_base)?;
        Some(p1[address.p1_index()].address()? + (usize::from(address) % Size4KiB::SIZE))
    }

    fn map<S, A>(&mut self, page: Page<S>, frame: Frame<S>, flags: Flags, allocator: &A) -> Result<(), PagingError>
    where
        S: FrameSize,
        A: FrameAllocator<Size4KiB>,
    {
        let physical_base = self.physical_base;

        if S::SIZE == Size4KiB::SIZE {
            let p1 = self
                .top_mut()
                .next_table_create(page.start.p3_index(), allocator, physical_base)?
                .next_table_create(page.start.p2_index(), allocator, physical_base)?;

            if p1[page.start.p1_index()].is_valid() {
                return Err(PagingError::AlreadyMapped);
            }

            p1[page.start.p1_index()].set(Some((frame.start, EntryFlags::from(flags))), true);
        } else if S::SIZE == Size2MiB::SIZE {
            let p2 = self.top_mut().next_table_create(page.start.p3_index(), allocator, physical_base)?;

            if p2[page.start.p2_index()].is_valid() {
                return Err(PagingError::AlreadyMapped);
            }

            p2[page.start.p2_index()].set(Some((frame.start, EntryFlags::from(flags))), true);
        } else {
            assert_eq!(S::SIZE, Size1GiB::SIZE);

            let p3 = self.top_mut();

            if p3[page.start.p3_index()].is_valid() {
                return Err(PagingError::AlreadyMapped);
            }

            p3[page.start.p3_index()].set(Some((frame.start, EntryFlags::from(flags))), true);
        }

        // TODO: replace this with a returned 'token' or whatever to batch changes before a flush if possible
        sfence_vma(None, Some(page.start));
        Ok(())
    }

    fn map_area<A>(
        &mut self,
        virtual_start: VAddr,
        physical_start: PAddr,
        size: usize,
        flags: Flags,
        allocator: &A,
    ) -> Result<(), PagingError>
    where
        A: FrameAllocator<Size4KiB>,
    {
        use mulch::math::{abs_difference, align_down};

        assert!(virtual_start.is_aligned(Size4KiB::SIZE));
        assert!(physical_start.is_aligned(Size4KiB::SIZE));
        assert!(size % Size4KiB::SIZE == 0);

        /*
         * If the area is smaller than a single 2MiB page, or if the virtual and physical starts are "out of
         * phase" such that we'll never be able to use larger pages, just use 4KiB pages.
         */
        let align_mismatch =
            abs_difference(usize::from(physical_start), usize::from(virtual_start)) % Size2MiB::SIZE != 0;
        if size < Size2MiB::SIZE || align_mismatch {
            let pages = Page::starts_with(virtual_start)..Page::starts_with(virtual_start + size);
            let frames = Frame::starts_with(physical_start)..Frame::starts_with(physical_start + size);
            return self.map_range::<Size4KiB, A>(pages, frames, flags, allocator);
        }

        let mut cursor = virtual_start;
        let virtual_end: VAddr = virtual_start + size;

        while cursor < virtual_end {
            let cursor_physical =
                PAddr::new(usize::from(physical_start) + usize::from(cursor) - usize::from(virtual_start))
                    .unwrap();
            let bytes_left = usize::from(virtual_end) - usize::from(cursor);

            if cursor.is_aligned(Size1GiB::SIZE)
                && cursor_physical.is_aligned(Size1GiB::SIZE)
                && bytes_left >= Size1GiB::SIZE
            {
                /*
                 * We can fit at least 1GiB page in, and both virtual and physical cursors have the correct
                 * alignment. Map as much as we can with 1GiB pages.
                 */
                let bytes_to_map = align_down(bytes_left, Size1GiB::SIZE);
                let pages = Page::starts_with(cursor)..Page::starts_with(cursor + bytes_to_map);
                let frames =
                    Frame::starts_with(cursor_physical)..Frame::starts_with(cursor_physical + bytes_to_map);
                self.map_range::<Size1GiB, A>(pages, frames, flags, allocator)?;
                cursor += bytes_to_map;
            } else if cursor.is_aligned(Size2MiB::SIZE)
                && cursor_physical.is_aligned(Size2MiB::SIZE)
                && bytes_left >= Size2MiB::SIZE
            {
                /*
                 * We couldn't use a 1GiB page, but we can use 2MiB pages! Map as much as we can.
                 *
                 * TODO: we could do a similar thing to below to check if we can use 1GiB pages further in, but
                 * it's probably unlikely enough that it's not really worth it.
                 */
                let bytes_to_map = align_down(bytes_left, Size2MiB::SIZE);
                let pages = Page::starts_with(cursor)..Page::starts_with(cursor + bytes_to_map);
                let frames =
                    Frame::starts_with(cursor_physical)..Frame::starts_with(cursor_physical + bytes_to_map);
                self.map_range::<Size2MiB, A>(pages, frames, flags, allocator)?;
                cursor += bytes_to_map;
            } else {
                /*
                 * We can't use any larger pages, but we might be able to further in, if the data becomes more
                 * aligned. If the next 2MiB-aligned address is still inside the range, stop there to have another
                 * go.
                 * NOTE: `cursor` might be 2MiB-aligned at this location, so we start from the next address so we don't get stuck here.
                 */
                let next_boundary = (cursor + 1).align_up(Size2MiB::SIZE);
                // Make sure not to go past the end of the region
                let bytes_to_map = cmp::min(
                    usize::from(next_boundary) - usize::from(cursor),
                    usize::from(virtual_end) - usize::from(cursor),
                );
                let pages = Page::starts_with(cursor)..Page::starts_with(cursor + bytes_to_map);
                let frames =
                    Frame::starts_with(cursor_physical)..Frame::starts_with(cursor_physical + bytes_to_map);
                self.map_range::<Size4KiB, A>(pages, frames, flags, allocator)?;
                cursor += bytes_to_map;
            }
        }

        assert_eq!(cursor, virtual_end);
        Ok(())
    }

    fn unmap<S>(&mut self, page: Page<S>) -> Option<Frame<S>>
    where
        S: FrameSize,
    {
        let physical_base = self.physical_base;

        match S::SIZE {
            Size4KiB::SIZE => {
                let p1 = self
                    .top_mut()
                    .next_table_mut(page.start.p3_index(), physical_base)?
                    .next_table_mut(page.start.p2_index(), physical_base)?;
                let frame = Frame::starts_with(p1[page.start.p1_index()].address()?);
                p1[page.start.p1_index()].set(None, true);
                sfence_vma(None, Some(page.start));

                Some(frame)
            }
            Size2MiB::SIZE => unimplemented!(),
            Size1GiB::SIZE => unimplemented!(),

            _ => panic!("Unimplemented page size!"),
        }
    }
}

pub trait VAddrIndices {
    fn p4_index(self) -> usize;
    fn p3_index(self) -> usize;
    fn p2_index(self) -> usize;
    fn p1_index(self) -> usize;

    fn from_indices(p4: usize, p3: usize, p2: usize, p1: usize) -> VAddr;
}

impl VAddrIndices for VAddr {
    fn p4_index(self) -> usize {
        usize::from(self).get_bits(39..48)
    }

    fn p3_index(self) -> usize {
        usize::from(self).get_bits(30..39)
    }

    fn p2_index(self) -> usize {
        usize::from(self).get_bits(21..30)
    }

    fn p1_index(self) -> usize {
        usize::from(self).get_bits(12..21)
    }

    fn from_indices(p4: usize, p3: usize, p2: usize, p1: usize) -> VAddr {
        let mut address = 0;
        address.set_bits(12..21, p1);
        address.set_bits(21..30, p2);
        address.set_bits(30..39, p3);
        address.set_bits(39..48, p4);
        VAddr::new(address)
    }
}

#[inline(always)]
pub fn sfence_vma(asid: Option<usize>, addr: Option<VAddr>) {
    match (asid, addr) {
        (Some(asid), Some(addr)) => unsafe { asm!("sfence.vma {}, {}", in(reg) usize::from(addr), in(reg) asid) },
        (Some(asid), None) => unsafe { asm!("sfence.vma zero, {}", in(reg) asid) },
        (None, Some(addr)) => unsafe { asm!("sfence.vma {}, zero", in(reg) usize::from(addr)) },
        (None, None) => unsafe { asm!("sfence.vma") },
    }
}
