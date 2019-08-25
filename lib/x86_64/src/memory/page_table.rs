use super::{Frame, FrameAllocator, Page, PhysicalAddress, Size2MiB, Size4KiB, VirtualAddress};
use crate::hw::{registers::write_control_reg, tlb};
use bitflags::bitflags;
use core::{
    fmt,
    marker::PhantomData,
    ops::{Index, IndexMut},
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
    ) -> Result<&mut Table<L::NextLevel>, MapError>
    where
        A: FrameAllocator,
    {
        /*
         * There's a special case here, where we want to create a new page table, but there's
         * already a huge-page there (e.g. we want to create a P1 table to map some 4KiB pages
         * there, but it's already a 2MiB huge-page). We detect that here (at the moment, this
         * panics, but it *maybe* should propagate instead. Not sure we want the handling code to
         * have to deal with this case really though).
         */

        if self.next_table(index, physical_base).is_none() {
            /*
             * This entry is empty, so we create a new page table, zero it, and return that.
             */
            let flags = EntryFlags::default()
                | EntryFlags::WRITABLE
                | if user_accessible { EntryFlags::USER_ACCESSIBLE } else { EntryFlags::empty() };
            self.entries[index].set(allocator.allocate().start_address, flags);

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
                return Err(MapError::TriedToMapInHugePage);
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
    pub fn new(frame: Frame, physical_base: VirtualAddress) -> PageTable {
        let mut table = PageTable { p4_frame: frame, physical_base };
        table.ref_to_p4().zero();
        table
    }

    /// Create a `PageTable` from a `Frame` that already contains a P4. This is very unsafe because
    /// it assumes that the frame contains a valid page table, and that no other `PageTable`s
    /// currently exist that use this same backing frame (as calling `mapper` on both could lead to
    /// two mutable references aliasing the same data to exist, which is UB).
    pub unsafe fn from_frame(p4_frame: Frame, physical_base: VirtualAddress) -> PageTable {
        PageTable { p4_frame, physical_base }
    }

    pub fn mapper<'a>(&'a mut self) -> Mapper<'a> {
        Mapper { physical_base: self.physical_base, p4: self.ref_to_p4() }
    }

    fn ref_to_p4(&mut self) -> &mut Table<Level4> {
        unsafe { &mut *((self.physical_base + usize::from(self.p4_frame.start_address)).mut_ptr()) }
    }

    pub fn switch_to(&self) {
        unsafe {
            write_control_reg!(cr3, usize::from(self.p4_frame.start_address) as u64);
        }
    }
}

pub struct Mapper<'a> {
    pub physical_base: VirtualAddress,
    pub p4: &'a mut Table<Level4>,
}

impl<'a> Mapper<'a> {
    pub fn translate(&self, address: VirtualAddress) -> TranslationResult {
        let p2 = match self
            .p4
            .next_table(address.p4_index(), self.physical_base)
            .and_then(|p3| p3.next_table(address.p3_index(), self.physical_base))
        {
            Some(p2) => p2,
            None => return TranslationResult::NotMapped,
        };

        let p2_entry = p2[address.p2_index()];
        if p2_entry.flags().contains(EntryFlags::HUGE_PAGE) {
            return TranslationResult::Frame2MiB(Frame::<Size2MiB>::starts_with(p2_entry.address().unwrap()));
        }

        let p1 = match p2.next_table(address.p2_index(), self.physical_base) {
            Some(p1) => p1,
            None => return TranslationResult::NotMapped,
        };

        match p1[address.p1_index()].address() {
            Some(address) => TranslationResult::Frame4KiB(Frame::<Size4KiB>::starts_with(address)),
            None => TranslationResult::NotMapped,
        }
    }

    /// Allocates a `Frame` using the given allocator, and maps the specified `Page` into it.
    /// Useful for when you need to map a page into physical memory, but you don't care where.
    /// Returns the allocated `Frame` so it can be freed when no longer needed by the caller.
    pub fn map<A>(
        &mut self,
        page: Page<Size4KiB>,
        flags: EntryFlags,
        allocator: &A,
    ) -> Result<Frame, MapError>
    where
        A: FrameAllocator,
    {
        let frame = allocator.allocate();
        self.map_to(page, frame, flags, allocator)?;
        Ok(frame)
    }

    pub fn map_to<A>(
        &mut self,
        page: Page<Size4KiB>,
        frame: Frame<Size4KiB>,
        flags: EntryFlags,
        allocator: &A,
    ) -> Result<(), MapError>
    where
        A: FrameAllocator,
    {
        /*
         * If a page should be accessible from userspace, all the parent paging structures for
         * that page must also be marked user-accessible.
         */
        let user_accessible = flags.contains(EntryFlags::USER_ACCESSIBLE);
        let p1 = self
            .p4
            .next_table_create(page.start_address.p4_index(), user_accessible, allocator, self.physical_base)?
            .next_table_create(page.start_address.p3_index(), user_accessible, allocator, self.physical_base)?
            .next_table_create(
                page.start_address.p2_index(),
                user_accessible,
                allocator,
                self.physical_base,
            )?;

        if !p1[page.start_address.p1_index()].is_unused() {
            return Err(MapError::AlreadyMapped);
        }

        p1[page.start_address.p1_index()].set(frame.start_address, flags | EntryFlags::default());
        // TODO: we could return a marker that the TLB must be flushed to avoid doing it in certain
        // instances when we e.g know we're going to change CR3 before accessing the new mappings.
        // This is fine for now though
        tlb::invalidate_page(page.start_address);
        Ok(())
    }

    #[allow(non_snake_case)]
    pub fn map_to_2MiB<A>(
        &mut self,
        page: Page<Size2MiB>,
        frame: Frame<Size2MiB>,
        flags: EntryFlags,
        allocator: &A,
    ) -> Result<(), MapError>
    where
        A: FrameAllocator,
    {
        /*
         * If a page should be accessible from userspace, all the parent paging structures for
         * that page must also be marked user-accessible.
         */
        let user_accessible = flags.contains(EntryFlags::USER_ACCESSIBLE);
        let p2 = self
            .p4
            .next_table_create(page.start_address.p4_index(), user_accessible, allocator, self.physical_base)?
            .next_table_create(
                page.start_address.p3_index(),
                user_accessible,
                allocator,
                self.physical_base,
            )?;

        if !p2[page.start_address.p2_index()].is_unused() {
            return Err(MapError::AlreadyMapped);
        }

        p2[page.start_address.p2_index()]
            .set(frame.start_address, flags | EntryFlags::HUGE_PAGE | EntryFlags::default());
        // TODO: we could return a marker that the TLB must be flushed to avoid doing it in certain
        // instances when we e.g know we're going to change CR3 before accessing the new mappings.
        // This is fine for now though
        tlb::invalidate_page(page.start_address);
        Ok(())
    }

    /// Unmap the given page, returning the `Frame` it was mapped to so the caller can choose to
    /// free it if needed. Returns `None` if the given page is not mapped.
    pub fn unmap(&mut self, page: Page<Size4KiB>) -> Option<Frame<Size4KiB>> {
        let p1 = self
            .p4
            .next_table_mut(page.start_address.p4_index(), self.physical_base)?
            .next_table_mut(page.start_address.p3_index(), self.physical_base)?
            .next_table_mut(page.start_address.p2_index(), self.physical_base)?;
        let frame = Frame::starts_with(p1[page.start_address.p1_index()].address()?);
        p1[page.start_address.p1_index()].set_unused();
        tlb::invalidate_page(page.start_address);

        Some(frame)
    }
}

#[derive(Debug)]
pub enum TranslationResult {
    Frame4KiB(Frame<Size4KiB>),
    Frame2MiB(Frame<Size2MiB>),
    NotMapped,
}

#[derive(Debug)]
pub enum MapError {
    AlreadyMapped,

    /// Produced when we tried to create a new page table, but there was already a huge page there
    /// (e.g. we needed to create a new P1 table, but there was a 2MiB page entry in the P2 at that
    /// index).
    TriedToMapInHugePage,
}
