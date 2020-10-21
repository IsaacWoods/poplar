use crate::hw::{registers::write_control_reg, tlb};
use bit_field::BitField;
use bitflags::bitflags;
use core::{
    fmt,
    marker::PhantomData,
    ops::{Index, IndexMut},
};
use hal::memory::{
    Flags,
    Frame,
    FrameAllocator,
    FrameSize,
    Page,
    PageTable,
    PagingError,
    PhysicalAddress,
    Size1GiB,
    Size2MiB,
    Size4KiB,
    VirtualAddress,
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

    pub fn flags(&self) -> EntryFlags {
        EntryFlags::from_bits_truncate(self.0)
    }

    pub fn address(&self) -> Option<PhysicalAddress> {
        if self.flags().contains(EntryFlags::PRESENT) {
            const ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;
            Some(PhysicalAddress::new((self.0 & ADDRESS_MASK) as usize).unwrap())
        } else {
            None
        }
    }

    /// Set an entry to have a particular mapping. Passing `None` will set this entry as not-present, whereas
    /// passing `Some` with a physical address and set of flags will populate an entry.
    pub fn set(&mut self, entry: Option<(PhysicalAddress, EntryFlags)>) {
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
    pub fn next_table(&self, index: usize, physical_base: VirtualAddress) -> Option<&Table<L::NextLevel>> {
        self[index]
            .address()
            .map(|physical_address| physical_base + usize::from(physical_address))
            .map(|virtual_address| unsafe { &*(virtual_address.ptr()) })
    }

    /// Get a mutable reference to the table at the given `index`, assuming the entirity of
    /// the physical address space is mapped from `physical_base`.
    pub fn next_table_mut(
        &mut self,
        index: usize,
        physical_base: VirtualAddress,
    ) -> Option<&mut Table<L::NextLevel>> {
        self[index]
            .address()
            .map(|physical_address| physical_base + usize::from(physical_address))
            .map(|virtual_address| unsafe { &mut *(virtual_address.mut_ptr()) })
    }

    pub fn next_table_create<A>(
        &mut self,
        index: usize,
        allocator: &A,
        physical_base: VirtualAddress,
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
    physical_base: VirtualAddress,
}

impl PageTableImpl {
    pub fn new(p4_frame: Frame, physical_base: VirtualAddress) -> PageTableImpl {
        let mut table = PageTableImpl { p4_frame, physical_base };
        Self::p4_mut(&mut table.p4_frame, table.physical_base).zero();
        table
    }

    /// Create a `PageTableImpl` from a `Frame` that already contains a P4. This is very unsafe because
    /// it assumes that the frame contains a valid page table, and that no other `PageTableImpl`s
    /// currently exist that use this same backing frame (as calling `mapper` on both could lead to
    /// two mutable references aliasing the same data to exist, which is UB).
    pub unsafe fn from_frame(p4_frame: Frame, physical_base: VirtualAddress) -> PageTableImpl {
        PageTableImpl { p4_frame, physical_base }
    }

    fn p4(&self) -> &Table<Level4> {
        unsafe { &*((self.physical_base + usize::from(self.p4_frame.start)).mut_ptr()) }
    }

    /// Get a mutable reference to the P4 table of this set of page tables. This can't take a `&mut self` like
    /// you'd normally write this, because then we borrow the entire struct and so can't access `physical_base`
    /// nicely. Instead, we mutably borrow the P4 frame to "represent" the borrow.
    fn p4_mut(frame: &mut Frame, physical_base: VirtualAddress) -> &mut Table<Level4> {
        unsafe { &mut *((physical_base + usize::from(frame.start)).mut_ptr()) }
    }
}

impl PageTable<Size4KiB> for PageTableImpl {
    fn new_with_kernel_mapped<A>(kernel_page_table: &Self, allocator: &A) -> Self
    where
        A: FrameAllocator<Size4KiB>,
    {
        let mut page_table = PageTableImpl::new(allocator.allocate(), crate::kernel_map::PHYSICAL_MAPPING_BASE);

        /*
         * Install the address of the kernel's P3 in every address space, so that the kernel is always mapped.
         * It's safe to unwrap the kernel P3 address, as we wouldn't be able to fetch these instructions
         * if it wasn't there.
         */
        let kernel_p3_address = kernel_page_table.p4()[crate::kernel_map::KERNEL_P4_ENTRY].address().unwrap();
        Self::p4_mut(&mut page_table.p4_frame, page_table.physical_base)[crate::kernel_map::KERNEL_P4_ENTRY]
            .set(Some((kernel_p3_address, EntryFlags::WRITABLE)));

        page_table
    }

    fn switch_to(&self) {
        unsafe {
            write_control_reg!(cr3, usize::from(self.p4_frame.start) as u64);
        }
    }

    fn translate(&self, address: VirtualAddress) -> Option<PhysicalAddress> {
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
        if S::SIZE == Size4KiB::SIZE {
            let p1 = Self::p4_mut(&mut self.p4_frame, self.physical_base)
                .next_table_create(page.start.p4_index(), allocator, self.physical_base)?
                .next_table_create(page.start.p3_index(), allocator, self.physical_base)?
                .next_table_create(page.start.p2_index(), allocator, self.physical_base)?;

            if !p1[page.start.p1_index()].is_unused() {
                return Err(PagingError::AlreadyMapped);
            }

            p1[page.start.p1_index()].set(Some((frame.start, EntryFlags::from(flags))));
        } else if S::SIZE == Size2MiB::SIZE {
            let p2 = Self::p4_mut(&mut self.p4_frame, self.physical_base)
                .next_table_create(page.start.p4_index(), allocator, self.physical_base)?
                .next_table_create(page.start.p3_index(), allocator, self.physical_base)?;

            if !p2[page.start.p2_index()].is_unused() {
                return Err(PagingError::AlreadyMapped);
            }

            p2[page.start.p2_index()].set(Some((frame.start, EntryFlags::from(flags) | EntryFlags::HUGE_PAGE)));
        } else {
            assert_eq!(S::SIZE, Size1GiB::SIZE);

            let p3 = Self::p4_mut(&mut self.p4_frame, self.physical_base).next_table_create(
                page.start.p4_index(),
                allocator,
                self.physical_base,
            )?;

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
        virtual_start: VirtualAddress,
        physical_start: PhysicalAddress,
        size: usize,
        flags: Flags,
        allocator: &A,
    ) -> Result<(), PagingError>
    where
        A: FrameAllocator<Size4KiB>,
    {
        use pebble_util::math::{abs_difference, align_down};

        assert!(virtual_start.is_aligned(Size4KiB::SIZE));
        assert!(physical_start.is_aligned(Size4KiB::SIZE));
        assert!(size % Size4KiB::SIZE == 0);

        /*
         * If the area is smaller than a single 2MiB page, or if the alignment of the physical and virtual regions
         * means we'll never be able to use larger pages, just map the whole area with 4KiB pages.
         */
        let align_mismatch =
            abs_difference(usize::from(physical_start), usize::from(virtual_start)) % Size2MiB::SIZE != 0;
        if size < Size2MiB::SIZE || align_mismatch {
            let pages = Page::starts_with(virtual_start)..Page::starts_with(virtual_start + size);
            let frames = Frame::starts_with(physical_start)..Frame::starts_with(physical_start + size);
            return self.map_range::<Size4KiB, A>(pages, frames, flags, allocator);
        }

        let mut cursor = virtual_start;
        while cursor < (virtual_start + size) {
            let cursor_physical = PhysicalAddress::new(
                usize::from(physical_start) + usize::from(cursor) - usize::from(virtual_start),
            )
            .unwrap();
            let bytes_left = size - (usize::from(cursor) - usize::from(virtual_start));

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
                let bytes_to_map = if next_boundary <= (virtual_start + size) {
                    bytes_left - (usize::from(virtual_start) + size - usize::from(next_boundary))
                } else {
                    bytes_left
                };
                let pages = Page::starts_with(cursor)..Page::starts_with(cursor + bytes_to_map);
                let frames =
                    Frame::starts_with(cursor_physical)..Frame::starts_with(cursor_physical + bytes_to_map);
                self.map_range::<Size4KiB, A>(pages, frames, flags, allocator)?;
                cursor += bytes_to_map;
            }
        }

        assert_eq!(cursor, virtual_start + size);
        Ok(())
    }

    fn unmap<S>(&mut self, page: Page<S>) -> Option<Frame<S>>
    where
        S: FrameSize,
    {
        match S::SIZE {
            Size4KiB::SIZE => {
                let p1 = Self::p4_mut(&mut self.p4_frame, self.physical_base)
                    .next_table_mut(page.start.p4_index(), self.physical_base)?
                    .next_table_mut(page.start.p3_index(), self.physical_base)?
                    .next_table_mut(page.start.p2_index(), self.physical_base)?;
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

pub trait VirtualAddressEx {
    fn p4_index(self) -> usize;
    fn p3_index(self) -> usize;
    fn p2_index(self) -> usize;
    fn p1_index(self) -> usize;
}

impl VirtualAddressEx for VirtualAddress {
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
}
