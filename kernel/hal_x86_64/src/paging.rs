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
    Mapper,
    MapperError,
    Page,
    PhysicalAddress,
    Size1GiB,
    Size2MiB,
    Size4KiB,
    VirtualAddress,
};

/// All page tables has 512 entries.
const ENTRY_COUNT: usize = 512;

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
            | if !flags.executable { EntryFlags::NO_EXECUTE } else { EntryFlags::empty() }
            | if flags.user_accessible { EntryFlags::USER_ACCESSIBLE } else { EntryFlags::empty() }
    }
}

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

    pub fn set_unused(&mut self) {
        self.0 = 0;
    }

    /// Set an entry to a given address and set of flags. Cannot be used to set an entry as
    /// not-present (use `set_unused` instead), because we automatically add the `PRESENT` flag.
    pub fn set(&mut self, address: PhysicalAddress, flags: EntryFlags) {
        self.0 = (usize::from(address) as u64) | (flags | EntryFlags::PRESENT).bits();
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

pub struct Table<L: TableLevel> {
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
            entry.set_unused();
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
        user_accessible: bool,
        allocator: &A,
        physical_base: VirtualAddress,
    ) -> Result<&mut Table<L::NextLevel>, MapperError>
    where
        A: FrameAllocator<Size4KiB>,
    {
        /*
         * There's a special case here, where we want to create a new page table, but there's
         * already a huge-page there (e.g. we want to create a P1 table to map some 4KiB pages
         * there, but it's already a 2MiB huge-page).
         */
        if self.next_table(index, physical_base).is_none() {
            /*
             * This entry is empty, so we create a new page table, zero it, and return that.
             */
            let flags = EntryFlags::default()
                | EntryFlags::WRITABLE
                | if user_accessible { EntryFlags::USER_ACCESSIBLE } else { EntryFlags::empty() };
            self.entries[index].set(allocator.allocate().start, flags);

            // Safe to unwrap because we just created the table there
            let table = self.next_table_mut(index, physical_base).unwrap();
            table.zero();
            Ok(table)
        } else {
            /*
             * A table already exists in the entry. This is actually the more difficult case - we
             * need to make sure the flags are suitable for both the existing sub-tables, and the
             * new ones, and also check it's not a huge-page.
             */
            if self[index].flags().contains(EntryFlags::HUGE_PAGE) {
                /*
                 * The entry is present, but is actually a huge page. It is **NOT** type-safe to
                 * call `next_table` on it. Instead, we return an error.
                 */
                return Err(MapperError::AlreadyMapped);
            }

            // TODO: find a set of flags suitable for both the existing entries and the new ones.
            // This needs a bit of thought: (e.g. NO_EXECUTE + NOT(NO_EXECUTE) => NOT(NO_EXECUTE)
            // but WRITABLE + NOT(WRITABLE) => WRITABLE so we basically need custom handling for
            // each flag). For the moment, we just return the table.
            //
            // NOTE: it's safe to alter the mappings for the parent structures, even if that makes
            // them more permissive than existing entries, because the final permissions are a
            // combination of the permissions of the parent tables and the final page. The parent
            // tables therefore need to be as permissive as any of the child tables.
            Ok(self.next_table_mut(index, physical_base).unwrap())
        }
    }
}

pub struct PageTable {
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

impl PageTable {
    pub fn new(p4_frame: Frame, physical_base: VirtualAddress) -> PageTable {
        let mut table = PageTable { p4_frame, physical_base };
        Self::p4_mut(&mut table.p4_frame, table.physical_base).zero();
        table
    }

    /// Create a `PageTable` from a `Frame` that already contains a P4. This is very unsafe because
    /// it assumes that the frame contains a valid page table, and that no other `PageTable`s
    /// currently exist that use this same backing frame (as calling `mapper` on both could lead to
    /// two mutable references aliasing the same data to exist, which is UB).
    pub unsafe fn from_frame(p4_frame: Frame, physical_base: VirtualAddress) -> PageTable {
        PageTable { p4_frame, physical_base }
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

impl<A> Mapper<Size4KiB, A> for PageTable
where
    A: FrameAllocator<Size4KiB>,
{
    fn new_for_address_space(kernel_page_table: &Self, allocator: &A) -> Self {
        let mut page_table = PageTable::new(allocator.allocate(), crate::kernel_map::PHYSICAL_MAPPING_BASE);

        /*
         * Install the address of the kernel's P3 in every address space, so that the kernel is always mapped.
         * It's safe to unwrap the kernel P3 address, as we wouldn't be able to fetch these instructions
         * if it wasn't there.
         */
        let kernel_p3_address = kernel_page_table.p4()[crate::kernel_map::KERNEL_P4_ENTRY].address().unwrap();
        Self::p4_mut(&mut page_table.p4_frame, page_table.physical_base)[crate::kernel_map::KERNEL_P4_ENTRY]
            .set(kernel_p3_address, EntryFlags::WRITABLE);

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

    fn map<S>(&mut self, page: Page<S>, frame: Frame<S>, flags: Flags, allocator: &A) -> Result<(), MapperError>
    where
        S: FrameSize,
    {
        /*
         * If a page should be accessible from userspace, all the parent paging structures for
         * that page must also be marked user-accessible.
         */
        // TODO: I think we should just pass all the flags upwards and amalgamalte them in `Flags` itself.
        let user_accessible = flags.user_accessible;
        if S::SIZE == Size4KiB::SIZE {
            let p1 = Self::p4_mut(&mut self.p4_frame, self.physical_base)
                .next_table_create(page.start.p4_index(), user_accessible, allocator, self.physical_base)?
                .next_table_create(page.start.p3_index(), user_accessible, allocator, self.physical_base)?
                .next_table_create(page.start.p2_index(), user_accessible, allocator, self.physical_base)?;

            if !p1[page.start.p1_index()].is_unused() {
                return Err(MapperError::AlreadyMapped);
            }

            p1[page.start.p1_index()].set(frame.start, EntryFlags::from(flags));
        } else if S::SIZE == Size2MiB::SIZE {
            let p2 = Self::p4_mut(&mut self.p4_frame, self.physical_base)
                .next_table_create(page.start.p4_index(), user_accessible, allocator, self.physical_base)?
                .next_table_create(page.start.p3_index(), user_accessible, allocator, self.physical_base)?;

            if !p2[page.start.p2_index()].is_unused() {
                return Err(MapperError::AlreadyMapped);
            }

            p2[page.start.p2_index()].set(frame.start, EntryFlags::from(flags) | EntryFlags::HUGE_PAGE);
        } else {
            // XXX: this needs to be implemented for any future implemented page sizes (e.g. 1GiB)
            unimplemented!()
        }

        // TODO: we could return a marker that the TLB must be flushed to avoid doing it in certain
        // instances when we e.g know we're going to change CR3 before accessing the new mappings.
        // This is fine for now though
        tlb::invalidate_page(page.start);
        Ok(())
    }

    fn map_area(
        &mut self,
        virtual_start: VirtualAddress,
        physical_start: PhysicalAddress,
        size: usize,
        flags: Flags,
        allocator: &A,
    ) -> Result<(), MapperError> {
        assert!(virtual_start.is_aligned(Size4KiB::SIZE));
        assert!(physical_start.is_aligned(Size4KiB::SIZE));
        assert!(size % Size4KiB::SIZE == 0);

        /*
         * Firstly, if the entire mapping is smaller than 2MiB, we simply map the entire thing with 4KiB pages.
         */
        if size < Size2MiB::SIZE {
            let pages =
                Page::<Size4KiB>::starts_with(virtual_start)..Page::<Size4KiB>::starts_with(virtual_start + size);
            let frames = Frame::<Size4KiB>::starts_with(physical_start)
                ..Frame::<Size4KiB>::starts_with(physical_start + size);
            self.map_range(pages, frames, flags, allocator)
        } else {
            /*
             * If it's larger, we split into three areas: a prefix, a middle, and a suffix. The prefix and
             * suffix are not aligned to 2MiB boundaries, and so must be mapped with 4KiB pages. The
             * middle is, and so can be mapped with larger 2MiB pages.
             */
            let virtual_prefix_start = virtual_start;
            let virtual_middle_start = virtual_start.align_up(Size2MiB::SIZE);
            let virtual_middle_end = (virtual_start + size).align_down(Size2MiB::SIZE);
            let virtual_suffix_end = virtual_start + size;

            let physical_prefix_start = physical_start;
            let physical_middle_start =
                physical_prefix_start + (usize::from(virtual_middle_start) - usize::from(virtual_prefix_start));
            let physical_middle_end =
                physical_prefix_start + (usize::from(virtual_middle_end) - usize::from(virtual_prefix_start));
            let physical_suffix_end = physical_start + size;

            // Map the prefix
            let prefix_pages = Page::<Size4KiB>::starts_with(virtual_prefix_start)
                ..Page::<Size4KiB>::starts_with(virtual_middle_start);
            let prefix_frames = Frame::<Size4KiB>::starts_with(physical_prefix_start)
                ..Frame::<Size4KiB>::starts_with(physical_middle_start);
            self.map_range(prefix_pages, prefix_frames, flags, allocator)?;

            // Map the middle
            let middle_pages = Page::<Size2MiB>::starts_with(virtual_middle_start)
                ..Page::<Size2MiB>::starts_with(virtual_middle_end);
            let middle_frames = Frame::<Size2MiB>::starts_with(physical_middle_start)
                ..Frame::<Size2MiB>::starts_with(physical_middle_end);
            self.map_range(middle_pages, middle_frames, flags, allocator)?;

            // Map the suffix
            let suffix_pages = Page::<Size4KiB>::starts_with(virtual_middle_end)
                ..Page::<Size4KiB>::starts_with(virtual_suffix_end);
            let suffix_frames = Frame::<Size4KiB>::starts_with(physical_middle_end)
                ..Frame::<Size4KiB>::starts_with(physical_suffix_end);
            self.map_range(suffix_pages, suffix_frames, flags, allocator)?;

            Ok(())
        }
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
                p1[page.start.p1_index()].set_unused();
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
