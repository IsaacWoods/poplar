pub mod addr;

pub use addr::{PAddr, VAddr};

use crate::cpu::Cr3;
use bit_field::BitField;
use core::{fmt, ops};

/// Poplar utilises 4-level paging on all x86_64 systems. This means the higher-half starts at
/// `0xffff_8000_0000_0000`, with the first half (64 TiB) dedicated to a direct mapping of the
/// physical address space.
///
/// The remaining portion of the higher half, from `0xffff_c000_0000_0000` is dynamically managed
/// for kernel allocations of virtual memory. The kernel image itself, as well as modules, boot
/// information, and other data loaded by the loader, starts at `-2GiB` (`0xffff_ffff_8000_0000`).
/// This is so it can use the `kernel` code model, which optimises for signed 32-bit immediates,
/// common in x86_64 instruction encodings.
pub mod kernel_map {
    use super::VAddr;

    pub const HIGHER_HALF_START: VAddr = VAddr::new(0xffff_8000_0000_0000);
    pub const PHYSICAL_MAPPING_BASE: VAddr = HIGHER_HALF_START;
    pub const KERNEL_DYNAMIC_BASE: VAddr = VAddr::new(0xffff_c000_0000_0000);
    pub const KERNEL_IMAGE_BASE: VAddr = VAddr::new(0xffff_ffff_8000_0000);
}

pub type Bytes = usize;
pub type Kibibytes = usize;
pub type Mebibytes = usize;
pub type Gibibytes = usize;

pub const fn kib(k: Kibibytes) -> Bytes {
    k * 1024
}
pub const fn mib(m: Mebibytes) -> Bytes {
    m * 1024 * 1024
}
pub const fn gib(g: Gibibytes) -> Bytes {
    g * 1024 * 1024 * 1024
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MemFlags {
    pub writable: bool,
    pub executable: bool,
    pub user_accessible: bool,
    pub cached: bool,
}

impl Default for MemFlags {
    fn default() -> Self {
        Self {
            writable: false,
            executable: false,
            user_accessible: false,
            cached: true,
        }
    }
}

pub const PAGE_TABLE_ENTRY_COUNT: usize = 512;

/// Represents a set of page tables. All page tables are manipulated in the context of having a full
/// mapping of the physical address space in virtual memory, or in an environment utilising an
/// identity mapping. Hence, page table entries occur through a `virtual_mapping_base`, which
/// corresponds to the bottom of the physical mapping.
pub struct PageTable {
    pub p4: PAddr,
    pub virtual_mapping_base: VAddr,
}

impl PageTable {
    pub const PAGE_SIZE_4KIB: Bytes = kib(4);
    pub const PAGE_SIZE_2MIB: Bytes = mib(2);
    pub const PAGE_SIZE_1GIB: Bytes = gib(1);

    pub fn new(allocator: &impl PageTableAllocator, virtual_mapping_base: VAddr) -> PageTable {
        let p4 = allocator.alloc();
        PageTable {
            p4,
            virtual_mapping_base,
        }
    }

    pub unsafe fn current(virtual_mapping_base: VAddr) -> PageTable {
        let mut cr3 = Cr3::read();
        cr3.set_bits(0..12, 0);

        PageTable {
            p4: PAddr::new(cr3 as usize),
            virtual_mapping_base,
        }
    }

    pub fn p4(&self) -> &Table {
        unsafe { &*((self.virtual_mapping_base + usize::from(self.p4)).ptr() as *const Table) }
    }

    pub fn p4_mut(&mut self) -> &mut Table {
        unsafe {
            &mut *((self.virtual_mapping_base + usize::from(self.p4)).mut_ptr() as *mut Table)
        }
    }

    /// Map a single frame. `size` must be one of 4KiB, 2MiB, or 1GiB. `virt` and `phys` must be
    /// appropriately aligned to the desired frame size.
    pub fn map_one(
        &mut self,
        virt: VAddr,
        phys: PAddr,
        size: usize,
        flags: MemFlags,
        allocator: &impl PageTableAllocator,
    ) -> Result<(), PageTableError> {
        assert!(usize::from(virt) % size == 0);
        assert!(usize::from(phys) % size == 0);

        let virtual_mapping_base = self.virtual_mapping_base;
        match size {
            Self::PAGE_SIZE_4KIB => {
                let p1 = self
                    .p4_mut()
                    .next_table_or_create(virt.p4_index(), virtual_mapping_base, allocator)
                    .next_table_or_create(virt.p3_index(), virtual_mapping_base, allocator)
                    .next_table_or_create(virt.p2_index(), virtual_mapping_base, allocator);
                if p1[virt.p1_index()].get(TableEntry::PRESENT) {
                    return Err(PageTableError::AddressAlreadyMapped);
                }
                p1[virt.p1_index()] = TableEntry::new()
                    .with(TableEntry::PRESENT, true)
                    .with(TableEntry::WRITABLE, flags.writable)
                    .with(TableEntry::USER, flags.user_accessible)
                    .with(TableEntry::WRITE_THROUGH, true)
                    .with(TableEntry::CACHE_DISABLE, !flags.cached)
                    .with_address(phys)
                    .with(TableEntry::EXECUTE_DISABLE, !flags.executable);
            }
            Self::PAGE_SIZE_2MIB => {
                let p2 = self
                    .p4_mut()
                    .next_table_or_create(virt.p4_index(), virtual_mapping_base, allocator)
                    .next_table_or_create(virt.p3_index(), virtual_mapping_base, allocator);
                if p2[virt.p2_index()].get(TableEntry::PRESENT) {
                    return Err(PageTableError::AddressAlreadyMapped);
                }
                p2[virt.p2_index()] = TableEntry::new()
                    .with(TableEntry::PRESENT, true)
                    .with(TableEntry::WRITABLE, flags.writable)
                    .with(TableEntry::USER, flags.user_accessible)
                    .with(TableEntry::WRITE_THROUGH, true)
                    .with(TableEntry::CACHE_DISABLE, !flags.cached)
                    .with(TableEntry::HUGE_PAGE, true)
                    .with_address(phys)
                    .with(TableEntry::EXECUTE_DISABLE, !flags.executable);
            }
            Self::PAGE_SIZE_1GIB => {
                let p3 = self.p4_mut().next_table_or_create(
                    virt.p4_index(),
                    virtual_mapping_base,
                    allocator,
                );
                if p3[virt.p3_index()].get(TableEntry::PRESENT) {
                    return Err(PageTableError::AddressAlreadyMapped);
                }
                p3[virt.p3_index()] = TableEntry::new()
                    .with(TableEntry::PRESENT, true)
                    .with(TableEntry::WRITABLE, flags.writable)
                    .with(TableEntry::USER, flags.user_accessible)
                    .with(TableEntry::WRITE_THROUGH, true)
                    .with(TableEntry::CACHE_DISABLE, !flags.cached)
                    .with(TableEntry::HUGE_PAGE, true)
                    .with_address(phys)
                    .with(TableEntry::EXECUTE_DISABLE, !flags.executable);
            }
            _ => panic!(),
        }

        Ok(())
    }

    pub fn map(
        &mut self,
        virt: VAddr,
        phys: PAddr,
        size: usize,
        flags: MemFlags,
        allocator: &impl PageTableAllocator,
    ) -> Result<(), PageTableError> {
        assert!(virt.is_aligned(Self::PAGE_SIZE_4KIB));
        assert!(phys.is_aligned(Self::PAGE_SIZE_4KIB));
        assert!(size % Self::PAGE_SIZE_4KIB == 0);

        /*
         * If the area to be mapped is smaller than a single 2MiB page, or if the virtual and
         * physical addresses are "out of phase" such that we'll never be able to use larger pages,
         * just map the region with 4KiB ones.
         */
        let align_mismatch =
            (usize::from(phys).abs_diff(usize::from(virt))) % Self::PAGE_SIZE_2MIB != 0;
        if size < Self::PAGE_SIZE_2MIB || align_mismatch {
            for i in 0..(size / Self::PAGE_SIZE_4KIB) {
                self.map_one(
                    virt + i * Self::PAGE_SIZE_4KIB,
                    phys + i * Self::PAGE_SIZE_4KIB,
                    Self::PAGE_SIZE_4KIB,
                    flags,
                    allocator,
                )?;
            }
            return Ok(());
        }

        let mut cursor = virt;
        while cursor < (virt + size) {
            let cursor_phys =
                PAddr::new(usize::from(phys) + usize::from(cursor) - usize::from(virt));
            let bytes_left = usize::from(virt) + usize::from(size) - usize::from(cursor);

            if cursor.is_aligned(Self::PAGE_SIZE_1GIB)
                && cursor_phys.is_aligned(Self::PAGE_SIZE_1GIB)
                && bytes_left >= Self::PAGE_SIZE_1GIB
            {
                self.map_one(cursor, cursor_phys, Self::PAGE_SIZE_1GIB, flags, allocator)?;
                cursor += Self::PAGE_SIZE_1GIB;
            } else if cursor.is_aligned(Self::PAGE_SIZE_2MIB)
                && cursor_phys.is_aligned(Self::PAGE_SIZE_2MIB)
                && bytes_left >= Self::PAGE_SIZE_2MIB
            {
                self.map_one(cursor, cursor_phys, Self::PAGE_SIZE_2MIB, flags, allocator)?;
                cursor += Self::PAGE_SIZE_2MIB;
            } else {
                self.map_one(cursor, cursor_phys, Self::PAGE_SIZE_4KIB, flags, allocator)?;
                cursor += Self::PAGE_SIZE_4KIB;
            }
        }

        assert_eq!(cursor, virt + size);
        Ok(())
    }
}

/// The `Debug` implementation of `PageTable` attempts to traverse the set of page tables to print
/// it in a tree-like form.
///
/// Flags format:
///     W U T C A D H G X
///     │ │ │ │ │ │ │ │ └ EXECUTE_DISABLE
///     │ │ │ │ │ │ │ └── GLOBAL
///     │ │ │ │ │ │ └──── HUGE_PAGE
///     │ │ │ │ │ └────── DIRTY
///     │ │ │ │ └──────── ACCESSED
///     │ │ │ └────────── CACHE_DISABLE
///     │ │ └──────────── WRITE_THROUGH
///     │ └────────────── USER
///     └──────────────── WRITABLE
impl fmt::Debug for PageTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct ShortFlags(TableEntry);
        impl fmt::Display for ShortFlags {
            #[rustfmt::skip]
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(
                    f,
                    "{}{}{}{}{}{}{}{}{}",
                    if self.0.get(TableEntry::WRITABLE) { 'W' } else { '-' },
                    if self.0.get(TableEntry::USER) { 'U' } else { '-' },
                    if self.0.get(TableEntry::WRITE_THROUGH) { 'T' } else { '-' },
                    if self.0.get(TableEntry::CACHE_DISABLE) { 'C' } else { '-' },
                    if self.0.get(TableEntry::ACCESSED) { 'A' } else { '-' },
                    if self.0.get(TableEntry::DIRTY) { 'D' } else { '-' },
                    if self.0.get(TableEntry::HUGE_PAGE) { 'H' } else { '-' },
                    if self.0.get(TableEntry::GLOBAL) { 'G' } else { '-' },
                    if self.0.get(TableEntry::EXECUTE_DISABLE) { 'X' } else { '-' },
                )
            }
        }

        writeln!(f, "PageTable {{")?;
        let p4 = self.p4();
        for i in 0..PAGE_TABLE_ENTRY_COUNT {
            if p4[i].get(TableEntry::PRESENT) {
                writeln!(
                    f,
                    "    P4 entry {}({:#x}) --> {:#x} ({})",
                    i,
                    VAddr::from_indices(i, 0, 0, 0, 0),
                    p4[i].address(),
                    ShortFlags(p4[i])
                )?;
                if p4[i].get(TableEntry::HUGE_PAGE) {
                    continue;
                }
                let p3 = p4.next_table(i, self.virtual_mapping_base).unwrap();
                for j in 0..PAGE_TABLE_ENTRY_COUNT {
                    if p3[j].get(TableEntry::PRESENT) {
                        if p3[j].get(TableEntry::HUGE_PAGE) {
                            writeln!(
                                f,
                                "        P3 entry {}({:#x}): 1GiB @ {:#x} ({})",
                                j,
                                VAddr::from_indices(i, j, 0, 0, 0),
                                p3[j].address(),
                                ShortFlags(p3[j])
                            )?;
                        } else {
                            writeln!(
                                f,
                                "        P3 entry {}({:#x}) --> {:#x} ({})",
                                j,
                                VAddr::from_indices(i, j, 0, 0, 0),
                                p3[j].address(),
                                ShortFlags(p3[j])
                            )?;
                            let p2 = p3.next_table(j, self.virtual_mapping_base).unwrap();
                            for k in 0..PAGE_TABLE_ENTRY_COUNT {
                                if p2[k].get(TableEntry::PRESENT) {
                                    if p2[k].get(TableEntry::HUGE_PAGE) {
                                        writeln!(
                                            f,
                                            "            P2 entry {}({:#x}): 2MiB @ {:#x} ({})",
                                            k,
                                            VAddr::from_indices(i, j, k, 0, 0),
                                            p2[k].address(),
                                            ShortFlags(p2[k])
                                        )?;
                                    } else {
                                        writeln!(
                                            f,
                                            "            P2 entry {}({:#x}) --> {:#x} ({})",
                                            k,
                                            VAddr::from_indices(i, j, k, 0, 0),
                                            p2[k].address(),
                                            ShortFlags(p2[k])
                                        )?;
                                        let p1 =
                                            p2.next_table(k, self.virtual_mapping_base).unwrap();
                                        for m in 0..PAGE_TABLE_ENTRY_COUNT {
                                            if p1[m].get(TableEntry::PRESENT) {
                                                writeln!(
                                                    f,
                                                    "                P1 entry {}({:#x}): 4KiB @ {:#x} ({})",
                                                    m,
                                                    VAddr::from_indices(i, j, k, m, 0),
                                                    p1[m].address(),
                                                    ShortFlags(p1[m])
                                                )?;
                                            }
                                        }
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

#[repr(transparent)]
pub struct Table([TableEntry; PAGE_TABLE_ENTRY_COUNT]);

impl Table {
    fn next_table(&self, i: usize, virtual_mapping_base: VAddr) -> Option<&Table> {
        if self.0[i].get(TableEntry::PRESENT) {
            unsafe {
                Some(
                    &*((virtual_mapping_base + usize::from(self.0[i].address())).ptr()
                        as *const Table),
                )
            }
        } else {
            None
        }
    }

    fn next_table_or_create(
        &mut self,
        i: usize,
        virtual_mapping_base: VAddr,
        allocator: &impl PageTableAllocator,
    ) -> &mut Table {
        if self.0[i].get(TableEntry::PRESENT) {
            unsafe {
                &mut *((virtual_mapping_base + usize::from(self.0[i].address())).mut_ptr()
                    as *mut Table)
            }
        } else {
            let new_table = allocator.alloc();
            self.0[i] = TableEntry::new()
                .with(TableEntry::PRESENT, true)
                .with(TableEntry::WRITABLE, true)
                .with(TableEntry::USER, true)
                .with(TableEntry::WRITE_THROUGH, true)
                .with(TableEntry::ACCESSED, true)
                .with_address(new_table);
            unsafe {
                &mut *((virtual_mapping_base + usize::from(new_table)).mut_ptr() as *mut Table)
            }
        }
    }
}

impl ops::Index<usize> for Table {
    type Output = TableEntry;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}
impl ops::IndexMut<usize> for Table {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

bitfield::bitfield! {
    pub struct TableEntry<u64> {
        pub const PRESENT: bool;
        pub const WRITABLE: bool;
        pub const USER: bool;
        pub const WRITE_THROUGH: bool;
        pub const CACHE_DISABLE: bool;
        pub const ACCESSED: bool;
        pub const DIRTY: bool;
        pub const HUGE_PAGE: bool;
        pub const GLOBAL: bool;
        const _RESERVED0 = 3;
        pub const ADDRESS = 51;
        pub const EXECUTE_DISABLE: bool;
    }
}

impl TableEntry {
    pub fn address(&self) -> PAddr {
        PAddr::new(self.get::<usize>(Self::ADDRESS) << 12)
    }

    pub fn with_address(self, address: PAddr) -> Self {
        self.with(Self::ADDRESS, (usize::from(address) as u64) >> 12)
    }
}

pub trait PageTableAllocator {
    /// Allocate a frame for a new page table. The supplied frame **must** be zeroed.
    fn alloc(&self) -> PAddr;
    /// Free a frame previously allocated for use in a page table.
    fn free(&self, frame: PAddr);
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PageTableError {
    /// A mapping covering the requested region already exists
    AddressAlreadyMapped,
}
