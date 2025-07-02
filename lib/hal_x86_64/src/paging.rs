use crate::hw::{registers::write_control_reg, tlb};
use bit_field::BitField;
use bitflags::bitflags;
use core::{
    cmp,
    fmt,
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
    pub struct EntryFlags : u64 {
        const PRESENT           = 1 << 0;
        const WRITABLE          = 1 << 1;
        const USER_ACCESSIBLE   = 1 << 2;
        const WRITE_THROUGH     = 1 << 3;
        const NO_CACHE          = 1 << 4;
        const ACCESSED          = 1 << 5;
        const DIRTY             = 1 << 6;
        const HUGE_PAGE         = 1 << 7;
        const GLOBAL            = 1 << 8;
        const NO_EXECUTE        = 1 << 63;

        /// This is the set of flags used for all non-terminal page tables (e.g. the ones that contain other page tables,
        /// not actual page mappings). It is the most permissive set of flags, preventing us from having to make sure
        /// parent page tables have the correct permissions for a terminal mapping. The actual permissions are therefore
        /// always simply determined by just the flags of the entry in the terminal page table.
        const NON_TERMINAL_FLAGS = Self::PRESENT.bits | Self::WRITABLE.bits | Self::USER_ACCESSIBLE.bits;
    }
}

impl Default for EntryFlags {
    fn default() -> EntryFlags {
        EntryFlags::PRESENT
    }
}

impl From<Flags> for EntryFlags {
    fn from(flags: Flags) -> Self {
        EntryFlags::PRESENT
            | if flags.writable { EntryFlags::WRITABLE } else { EntryFlags::empty() }
            | if flags.executable { EntryFlags::empty() } else { EntryFlags::NO_EXECUTE }
            | if flags.user_accessible { EntryFlags::USER_ACCESSIBLE } else { EntryFlags::empty() }
            | if flags.cached { EntryFlags::empty() } else { EntryFlags::NO_CACHE }
    }
}

/// Represents an entry within a page table of any level. Contains a physical address to the next level (or to the
/// physical memory region), and some flags.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Entry(u64);

impl Entry {
    pub fn unused() -> Entry {
        Entry(0)
    }

    pub fn is_unused(&self) -> bool {
        self.0 == 0
    }

    pub fn is_present(&self) -> bool {
        self.flags().contains(EntryFlags::PRESENT)
    }

    pub fn flags(&self) -> EntryFlags {
        EntryFlags::from_bits_truncate(self.0)
    }

    pub fn address(&self) -> Option<PAddr> {
        if self.is_present() {
            const ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;
            Some(PAddr::new((self.0 & ADDRESS_MASK) as usize).unwrap())
        } else {
            None
        }
    }

    /// Set an entry to have a particular mapping. Passing `None` will set this entry as not-present, whereas
    /// passing `Some` with a physical address and set of flags will populate an entry.
    pub fn set(&mut self, entry: Option<(PAddr, EntryFlags)>) {
        self.0 = match entry {
            Some((address, flags)) => (usize::from(address) as u64) | (flags | EntryFlags::PRESENT).bits(),
            None => 0,
        };
    }
}

impl fmt::Debug for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.flags().contains(EntryFlags::PRESENT) {
            write!(f, "Not Present")
        } else {
            if self.flags().contains(EntryFlags::HUGE_PAGE) {
                write!(f, "[HUGE] Address: {:#x}, flags: {:?}", self.address().unwrap(), self.flags())
            } else {
                write!(f, "Address: {:#x}, flags: {:?}", self.address().unwrap(), self.flags())
            }
        }
    }
}

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
            entry.set(None);
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
            self.entries[index].set(Some((allocator.allocate().start, EntryFlags::NON_TERMINAL_FLAGS)));
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
            if self[index].flags().contains(EntryFlags::HUGE_PAGE) {
                return Err(PagingError::AlreadyMapped);
            }

            Ok(self.next_table_mut(index, physical_base).unwrap())
        }
    }
}

pub struct PageTableImpl {
    p4_frame: Frame,
    /// The virtual address at which physical memory is mapped in the environment that these page
    /// tables are being constructed in. This is **not** a property of the set of page tables being
    /// mapped. For example, in the bootloader, we construct a set of page tables for the kernel
    /// where physical memory is mapped in the top P4 entry, but `physical_base` is set to `0`
    /// because the UEFI sets up an identity-mapping for the bootloader. The same set of page
    /// tables would have a `physical_base` in the higher half in the kernel, after we switch to
    /// the kernel's set of page tables.
    physical_base: VAddr,
}

impl PageTableImpl {
    pub fn new(p4_frame: Frame, physical_base: VAddr) -> PageTableImpl {
        let mut table = PageTableImpl { p4_frame, physical_base };
        table.p4_mut().zero();
        table
    }

    /// Create a `PageTableImpl` from a `Frame` that already contains a P4. This is very unsafe because
    /// it assumes that the frame contains a valid page table, and that no other `PageTableImpl`s
    /// currently exist that use this same backing frame (as calling `mapper` on both could lead to
    /// two mutable references aliasing the same data to exist, which is UB).
    pub unsafe fn from_frame(p4_frame: Frame, physical_base: VAddr) -> PageTableImpl {
        PageTableImpl { p4_frame, physical_base }
    }

    pub fn p4(&self) -> &Table<Level4> {
        unsafe { &*((self.physical_base + usize::from(self.p4_frame.start)).ptr()) }
    }

    pub fn p4_mut(&mut self) -> &mut Table<Level4> {
        unsafe { &mut *((self.physical_base + usize::from(self.p4_frame.start)).mut_ptr()) }
    }
}

impl fmt::Debug for PageTableImpl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "PageTable {{")?;
        let p4 = self.p4();
        for i in 0..512 {
            if p4[i].is_present() {
                writeln!(f, "    P4 entry {}({:#x}): {:?}", i, VAddr::from_indices(i, 0, 0, 0), p4[i])?;
                if p4[i].flags().contains(EntryFlags::HUGE_PAGE) {
                    continue;
                }
                let p3 = p4.next_table(i, self.physical_base).unwrap();
                for j in 0..512 {
                    if p3[j].is_present() {
                        writeln!(
                            f,
                            "        P3 entry {}({:#x}): {:?}",
                            j,
                            VAddr::from_indices(i, j, 0, 0),
                            p3[j]
                        )?;
                        if p3[j].flags().contains(EntryFlags::HUGE_PAGE) {
                            continue;
                        }
                        let p2 = p3.next_table(j, self.physical_base).unwrap();
                        for k in 0..512 {
                            if p2[k].is_present() {
                                writeln!(
                                    f,
                                    "            P2 entry {}({:#x}): {:?}",
                                    k,
                                    VAddr::from_indices(i, j, k, 0),
                                    p2[k]
                                )?;
                                if p2[k].flags().contains(EntryFlags::HUGE_PAGE) {
                                    continue;
                                }
                                let p1 = p2.next_table(k, self.physical_base).unwrap();
                                for m in 0..512 {
                                    if p1[m].is_present() {
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

impl PageTable<Size4KiB> for PageTableImpl {
    fn new_with_kernel_mapped<A>(kernel_page_table: &Self, allocator: &A) -> Self
    where
        A: FrameAllocator<Size4KiB>,
    {
        // TODO: this should be done in the kernel bc it's not really a HAL concern how we lay out
        // memory
        const PHYSICAL_MAPPING_BASE: VAddr = VAddr::new(0xffff_8000_0000_0000);
        let mut page_table = PageTableImpl::new(allocator.allocate(), PHYSICAL_MAPPING_BASE);

        // /*
        //  * Install the address of the kernel's P3 in every address space, so that the kernel is always mapped.
        //  * It's safe to unwrap the kernel P3 address, as we wouldn't be able to fetch these instructions
        //  * if it wasn't there.
        //  */
        // let kernel_p3_address = kernel_page_table.p4()[crate::kernel_map::KERNEL_P4_ENTRY].address().unwrap();
        // page_table.p4_mut()[crate::kernel_map::KERNEL_P4_ENTRY]
        //     .set(Some((kernel_p3_address, EntryFlags::WRITABLE)));

        // TODO: this could be parameterised better I'm sure
        for i in 256..512 {
            let kernel_p3_address = kernel_page_table.p4()[i].address().unwrap();
            page_table.p4_mut()[i].set(Some((kernel_p3_address, EntryFlags::WRITABLE)));
        }

        page_table
    }

    unsafe fn switch_to(&self) {
        unsafe {
            write_control_reg!(cr3, usize::from(self.p4_frame.start) as u64);
        }
    }

    fn translate(&self, address: VAddr) -> Option<PAddr> {
        // TODO: handle huge pages at the P3 level as well

        let p2 = self
            .p4()
            .next_table(address.p4_index(), self.physical_base)
            .and_then(|p3| p3.next_table(address.p3_index(), self.physical_base))?;

        let p2_entry = p2[address.p2_index()];
        if p2_entry.flags().contains(EntryFlags::HUGE_PAGE) {
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
                .p4_mut()
                .next_table_create(page.start.p4_index(), allocator, physical_base)?
                .next_table_create(page.start.p3_index(), allocator, physical_base)?
                .next_table_create(page.start.p2_index(), allocator, physical_base)?;

            if !p1[page.start.p1_index()].is_unused() {
                return Err(PagingError::AlreadyMapped);
            }

            p1[page.start.p1_index()].set(Some((frame.start, EntryFlags::from(flags))));
        } else if S::SIZE == Size2MiB::SIZE {
            let p2 = self
                .p4_mut()
                .next_table_create(page.start.p4_index(), allocator, physical_base)?
                .next_table_create(page.start.p3_index(), allocator, physical_base)?;

            if !p2[page.start.p2_index()].is_unused() {
                return Err(PagingError::AlreadyMapped);
            }

            p2[page.start.p2_index()].set(Some((frame.start, EntryFlags::from(flags) | EntryFlags::HUGE_PAGE)));
        } else {
            assert_eq!(S::SIZE, Size1GiB::SIZE);

            let p3 = self.p4_mut().next_table_create(page.start.p4_index(), allocator, physical_base)?;

            if !p3[page.start.p3_index()].is_unused() {
                return Err(PagingError::AlreadyMapped);
            }

            p3[page.start.p3_index()].set(Some((frame.start, EntryFlags::from(flags) | EntryFlags::HUGE_PAGE)));
        }

        // TODO: we could return a marker that the TLB must be flushed to avoid doing it in certain
        // instances when we e.g know we're going to change CR3 before accessing the new mappings.
        // This is fine for now though
        tlb::invalidate_page(page.start);
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
                    .p4_mut()
                    .next_table_mut(page.start.p4_index(), physical_base)?
                    .next_table_mut(page.start.p3_index(), physical_base)?
                    .next_table_mut(page.start.p2_index(), physical_base)?;
                let frame = Frame::starts_with(p1[page.start.p1_index()].address()?);
                p1[page.start.p1_index()].set(None);
                tlb::invalidate_page(page.start);

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
        let mut address = 0usize;
        address.set_bits(12..21, p1);
        address.set_bits(21..30, p2);
        address.set_bits(30..39, p3);
        address.set_bits(39..48, p4);
        VAddr::new(address)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ops::Range;
    use hal::memory::FakeFrameAllocator;
    use std::collections::VecDeque;

    #[test]
    fn test_map_area_single_page() {
        let mut page_table = TestPageTable::new();
        page_table.add_expected_mapping::<Size4KiB>(0x4000_0000, 0x2000_0000);

        page_table
            .map_area(
                VAddr::new(0x4000_0000),
                PAddr::new(0x2000_0000).unwrap(),
                0x1000,
                Flags::default(),
                &FakeFrameAllocator,
            )
            .unwrap();
        page_table.ensure_all_mappings_made();
    }

    #[test]
    fn test_map_area_range() {
        let mut page_table = TestPageTable::new();
        page_table.add_expected_mapping::<Size4KiB>(0x4000_0000, 0x2000_f000);
        page_table.add_expected_mapping::<Size4KiB>(0x4000_1000, 0x2001_0000);
        page_table.add_expected_mapping::<Size4KiB>(0x4000_2000, 0x2001_1000);
        page_table.add_expected_mapping::<Size4KiB>(0x4000_3000, 0x2001_2000);
        page_table.add_expected_mapping::<Size4KiB>(0x4000_4000, 0x2001_3000);
        page_table
            .map_area(
                VAddr::new(0x4000_0000),
                PAddr::new(0x2000_f000).unwrap(),
                0x5000,
                Flags::default(),
                &FakeFrameAllocator,
            )
            .unwrap();
        page_table.ensure_all_mappings_made();

        // ----------
        page_table.add_expected_mapping::<Size2MiB>(0x6000_0000, 0x0);
        page_table.add_expected_mapping::<Size2MiB>(0x6020_0000, 0x20_0000);
        page_table
            .map_area(
                VAddr::new(0x6000_0000),
                PAddr::new(0x0).unwrap(),
                0x400000,
                Flags::default(),
                &FakeFrameAllocator,
            )
            .unwrap();
        page_table.ensure_all_mappings_made();
    }

    #[test]
    fn test_map_area_unaligned() {
        let mut page_table = TestPageTable::new();
        let virtual_start = 0x1000_1000;
        let physical_start = 0x2000_0000;
        let size = 0x205000;

        for address in (virtual_start..(virtual_start + size)).into_iter().step_by(0x1000) {
            page_table.add_expected_mapping::<Size4KiB>(address, physical_start + (address - virtual_start));
        }

        page_table
            .map_area(
                VAddr::new(virtual_start),
                PAddr::new(physical_start).unwrap(),
                size,
                Flags::default(),
                &FakeFrameAllocator,
            )
            .unwrap();
        page_table.ensure_all_mappings_made();
    }

    #[test]
    fn test_map_area_aligned() {
        let mut page_table = TestPageTable::new();
        page_table.add_expected_mapping::<Size2MiB>(0x1000_0000, 0x2000_0000);
        page_table.add_expected_mapping::<Size4KiB>(0x1020_0000, 0x2020_0000);
        page_table.add_expected_mapping::<Size4KiB>(0x1020_1000, 0x2020_1000);
        page_table.add_expected_mapping::<Size4KiB>(0x1020_2000, 0x2020_2000);
        page_table.add_expected_mapping::<Size4KiB>(0x1020_3000, 0x2020_3000);
        page_table.add_expected_mapping::<Size4KiB>(0x1020_4000, 0x2020_4000);

        page_table
            .map_area(
                VAddr::new(0x1000_0000),
                PAddr::new(0x2000_0000).unwrap(),
                0x205000,
                Flags::default(),
                &FakeFrameAllocator,
            )
            .unwrap();
        page_table.ensure_all_mappings_made();

        // ----------
        page_table.add_expected_mapping::<Size4KiB>(0x0fff_e000, 0x1fff_e000);
        page_table.add_expected_mapping::<Size4KiB>(0x0fff_f000, 0x1fff_f000);
        page_table.add_expected_mapping::<Size2MiB>(0x1000_0000, 0x2000_0000);
        page_table.add_expected_mapping::<Size4KiB>(0x1020_0000, 0x2020_0000);
        page_table.add_expected_mapping::<Size4KiB>(0x1020_1000, 0x2020_1000);
        page_table.add_expected_mapping::<Size4KiB>(0x1020_2000, 0x2020_2000);
        page_table.add_expected_mapping::<Size4KiB>(0x1020_3000, 0x2020_3000);
        page_table.add_expected_mapping::<Size4KiB>(0x1020_4000, 0x2020_4000);

        page_table
            .map_area(
                VAddr::new(0x0fff_e000),
                PAddr::new(0x1fff_e000).unwrap(),
                0x207000,
                Flags::default(),
                &FakeFrameAllocator,
            )
            .unwrap();
        page_table.ensure_all_mappings_made();
    }

    struct TestPageTable {
        expected_maps: VecDeque<(usize, VAddr, PAddr)>,
    }

    impl TestPageTable {
        pub fn new() -> Self {
            TestPageTable { expected_maps: VecDeque::new() }
        }

        pub fn add_expected_mapping<S>(&mut self, virtual_start: usize, physical_start: usize)
        where
            S: FrameSize,
        {
            self.expected_maps.push_back((
                S::SIZE,
                VAddr::new(virtual_start),
                PAddr::new(physical_start).unwrap(),
            ));
        }

        pub fn ensure_all_mappings_made(&self) {
            assert!(self.expected_maps.is_empty());
        }
    }

    impl PageTable<Size4KiB> for TestPageTable {
        fn new_with_kernel_mapped<A>(_kernel_page_table: &Self, _allocator: &A) -> Self
        where
            A: FrameAllocator<Size4KiB>,
        {
            unimplemented!()
        }

        unsafe fn switch_to(&self) {
            unimplemented!()
        }

        fn translate(&self, _address: VAddr) -> Option<PAddr> {
            unimplemented!()
        }

        fn map<S, A>(&mut self, page: Page<S>, frame: Frame<S>, flags: Flags, _: &A) -> Result<(), PagingError>
        where
            S: FrameSize,
            A: FrameAllocator<Size4KiB>,
        {
            println!(
                "Mapping {:#x} page at {:#x} to {:#x} with flags {:?}",
                S::SIZE,
                page.start,
                frame.start,
                flags
            );

            let (size, virt_start, phys_start) = self.expected_maps.pop_front().expect("Map not expected");
            assert_eq!(size, S::SIZE);
            assert_eq!(virt_start, page.start);
            assert_eq!(phys_start, frame.start);

            Ok(())
        }

        fn map_range<S, A>(
            &mut self,
            pages: Range<Page<S>>,
            frames: Range<Frame<S>>,
            flags: Flags,
            allocator: &A,
        ) -> Result<(), PagingError>
        where
            S: FrameSize,
            A: FrameAllocator<Size4KiB>,
        {
            println!(
                "Mapping range of {:#x} pages at {:#x}..{:#x} to {:#x}..{:#x} with flags {:?}",
                S::SIZE,
                pages.start.start,
                pages.end.start,
                frames.start.start,
                frames.end.start,
                flags
            );
            for (page, frame) in pages.zip(frames) {
                self.map(page, frame, flags, allocator)?;
            }

            Ok(())
        }

        // XXX: it's a shame we can't easily reuse the actual code in the test. Changes need to be reflected above
        // into the real code.
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
                println!(
                    "Small size or align_mismatch means we just use 4KiB pages (too_small = {}, align_mismatch = {})",
                    size < Size2MiB::SIZE, align_mismatch
                );
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
                    println!("Mapping {:#x} bytes using 1GiB pages", bytes_to_map);
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
                    println!("Mapping {:#x} bytes using 2MiB pages", bytes_to_map);
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
                    println!("Mapping {:#x} bytes using 4KiB pages", bytes_to_map);
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
            unimplemented!()
        }
    }
}
