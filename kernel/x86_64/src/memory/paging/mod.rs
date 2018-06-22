pub mod entry;
mod mapper;
mod physical_address;
mod table;
mod temporary_page;
mod virtual_address;

pub use self::entry::*;
pub use self::mapper::{Mapper, PhysicalMapping};
pub use self::physical_address::PhysicalAddress;
pub use self::temporary_page::TemporaryPage;
pub use self::virtual_address::VirtualAddress;

use super::map::RECURSIVE_ENTRY;
use super::{Frame, FrameAllocator};
use core::ops::{Add, Deref, DerefMut};
use multiboot2::BootInformation;
use tlb;

pub const PAGE_SIZE: usize = 4096;
pub const ENTRY_COUNT: usize = 512;

#[derive(Clone)]
pub struct PageIter {
    start: Page,
    end: Page,
}

impl Iterator for PageIter {
    type Item = Page;

    fn next(&mut self) -> Option<Page> {
        if self.start <= self.end {
            let page = self.start;
            self.start.number += 1;
            Some(page)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Page {
    pub(in memory) number: usize,
}

impl Add<usize> for Page {
    type Output = Page;

    fn add(self, rhs: usize) -> Self {
        Page {
            number: self.number + rhs,
        }
    }
}

impl Page {
    pub fn range_inclusive(start: Page, end: Page) -> PageIter {
        PageIter { start, end }
    }

    pub fn start_address(&self) -> VirtualAddress {
        (self.number * PAGE_SIZE).into()
    }

    pub fn containing_page(address: VirtualAddress) -> Page {
        assert!(
            address < 0x0000_8000_0000_0000.into() || address >= 0xffff_8000_0000_0000.into(),
            "Invalid address: {:#x}",
            address
        );

        Page {
            number: usize::from(address) / PAGE_SIZE,
        }
    }

    fn p4_index(&self) -> u16 {
        ((self.number >> 27) & 0o777) as u16
    }
    fn p3_index(&self) -> u16 {
        ((self.number >> 18) & 0o777) as u16
    }
    fn p2_index(&self) -> u16 {
        ((self.number >> 9) & 0o777) as u16
    }
    fn p1_index(&self) -> u16 {
        ((self.number >> 0) & 0o777) as u16
    }
}

pub struct ActivePageTable {
    mapper: Mapper,
}

impl Deref for ActivePageTable {
    type Target = Mapper;

    fn deref(&self) -> &Mapper {
        &self.mapper
    }
}

impl DerefMut for ActivePageTable {
    fn deref_mut(&mut self) -> &mut Mapper {
        &mut self.mapper
    }
}

impl ActivePageTable {
    pub unsafe fn new() -> ActivePageTable {
        ActivePageTable {
            mapper: Mapper::new(),
        }
    }

    /*
     * This uses a trick with the recursive mapping technique we use to alter an `InactivePageTable`,
     * by mapping its P4 address as if it were the active table's P4.
     *
     * By returning a `Mapper` to the closure, instead of an `ActivePageTable`, we stop it from
     * stop it from calling this `with` method again, which fails because the recursive mapping
     * wouldn't be set up correctly.
     *
     * XXX: Within the closure, the inactive page tables can be modified, but they are not mapped.
     *      To read or write to addresses mapped, they must be temporarily mapped into the active
     *      page tables.
     */
    pub fn with<F>(
        &mut self,
        table: &mut InactivePageTable,
        frame_allocator: &mut FrameAllocator,
        f: F,
    ) where
        F: FnOnce(&mut Mapper, &mut FrameAllocator),
    {
        let mut temporary_page = TemporaryPage::new(::memory::map::TEMP_PAGE);

        // Inner scope used to end the borrow of `temporary_page`
        {
            // Backup the current P4 and temporarily map it
            let original_p4 = Frame::containing_frame((read_control_reg!(cr3) as usize).into());
            let p4_table = temporary_page.map_table_frame(original_p4, self, frame_allocator);

            // Overwrite recursive mapping
            self.p4[RECURSIVE_ENTRY]
                .set(table.p4_frame, EntryFlags::PRESENT | EntryFlags::WRITABLE);

            // Flush the TLB
            tlb::flush();

            // Execute in the new context
            f(self, frame_allocator);

            // Restore recursive mapping to original P4
            p4_table[RECURSIVE_ENTRY].set(original_p4, EntryFlags::PRESENT | EntryFlags::WRITABLE);
            tlb::flush();
        }

        temporary_page.unmap(self, frame_allocator);
    }

    #[allow(needless_pass_by_value)] // We move the table, so it can't be used once it's not mapped
    pub fn switch(&mut self, new_table: InactivePageTable) -> ActivePageTable {
        unsafe {
            /*
             * NOTE: We don't need to flush the TLB here because the CPU does it automatically when
             *       CR3 is reloaded.
             */
            write_control_reg!(cr3, usize::from(new_table.p4_frame.start_address()) as u64);
        }

        unsafe { ActivePageTable::new() }
    }
}

pub struct InactivePageTable {
    p4_frame: Frame,
}

impl InactivePageTable {
    pub fn new(
        frame: Frame,
        active_table: &mut ActivePageTable,
        frame_allocator: &mut FrameAllocator,
    ) -> InactivePageTable {
        /*
         * We firstly temporarily map the page table into memory so we can zero it.
         * We then set up recursive mapping on the P4.
         *
         * NOTE: We use an inner scope here to make sure that `table` is dropped before
         *       we try to unmap the temporary page.
         */
        let mut temporary_page = TemporaryPage::new(::memory::map::TEMP_PAGE);

        {
            let table = temporary_page.map_table_frame(frame, active_table, frame_allocator);
            table.zero();
            table[RECURSIVE_ENTRY].set(frame, EntryFlags::PRESENT | EntryFlags::WRITABLE);
        }

        temporary_page.unmap(active_table, frame_allocator);
        InactivePageTable { p4_frame: frame }
    }
}

pub fn remap_kernel(
    boot_info: &BootInformation,
    frame_allocator: &mut FrameAllocator,
) -> ActivePageTable {
    use memory::map::KERNEL_VMA;

    // This represents the page tables created by the bootstrap
    let mut active_table = unsafe { ActivePageTable::new() };

    /*
     * We can now allocate space for a new set of page tables, then temporarily map it into memory
     * so we can create a new set of page tables.
     */
    let mut new_table = {
        let frame = frame_allocator.allocate_frame().expect("run out of frames");
        InactivePageTable::new(frame, &mut active_table, frame_allocator)
    };

    extern "C" {
        /*
         * The ADDRESS of this is the location of the guard page.
         */
        static _guard_page: u8;
    }
    let guard_page_addr: VirtualAddress = unsafe { (&_guard_page as *const u8).into() };
    assert!(
        guard_page_addr.is_page_aligned(),
        "Guard page address is not page aligned!"
    );

    /*
     * We now populate the new page tables for the kernel. We do this by installing the physical
     * address of the inactive P4 into the active P4's recursive entry, then mapping stuff as if we
     * were modifying the active tables, then switch to the real tables.
     */
    active_table.with(&mut new_table, frame_allocator, |mapper, allocator| {
        let elf_sections_tag = boot_info
            .elf_sections_tag()
            .expect("Memory map tag required");

        /*
         * Map the kernel sections with the correct permissions.
         */
        for section in elf_sections_tag.sections() {
            let section_start = VirtualAddress::new(section.start_address() as usize);
            let section_end = VirtualAddress::new(section.end_address() as usize);

            /*
             * Skip sections that either aren't to be allocated or are located before the start
             * of the the higher-half (and so are probably part of the bootstrap).
             */
            if !section.is_allocated() || !section_start.is_in_kernel_space() {
                continue;
            }

            assert!(
                section_start.is_page_aligned(),
                "sections must be page aligned"
            );
            trace!(
                "Allocating section: {} to {:#x}-{:#x}",
                // section.name(),
                "Potato", // FIXME: needs changes in `multiboot2`
                section_start,
                section_end
            );

            for page in Page::range_inclusive(
                Page::containing_page(section_start),
                Page::containing_page(section_end.offset(-1)),
            ) {
                let physical_address = PhysicalAddress::new(
                    usize::from(page.start_address()) - usize::from(KERNEL_VMA),
                );

                mapper.map_to(
                    page,
                    Frame::containing_frame(physical_address),
                    EntryFlags::from_elf_section(&section),
                    allocator,
                );
            }
        }

        /*
         * Map the Multiboot structure to KERNEL_VMA + its physical address
         */
        let multiboot_start = VirtualAddress::new(boot_info.start_address());
        let multiboot_end = VirtualAddress::new(boot_info.end_address());
        trace!(
            "Mapping Multiboot structure to {:#x}-{:#x}",
            multiboot_start,
            multiboot_end
        );
        mapper.identity_map_range(
            PhysicalAddress::from_kernel_space(multiboot_start)
                ..PhysicalAddress::from_kernel_space(multiboot_end),
            EntryFlags::PRESENT,
            allocator,
        );

        /*
         * Unmap the stack's guard page. This stops us overflowing the stack by causing a page
         * fault if we try to access the memory directly above the stack.
         */
        mapper.unmap(Page::containing_page(guard_page_addr), allocator);
    });

    active_table.switch(new_table);
    active_table
}
