/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

pub mod entry; // TODO: It isn't ideal to have this public, move reponsibility for this inside mod
mod table;
mod temporary_page;
mod temporary_vec;
mod mapper;

pub use self::entry::*;

use core::ops::{Add,Deref,DerefMut};
use self::mapper::Mapper;
use self::temporary_vec::TemporaryVec;
use self::temporary_page::TemporaryPage;
use memory::{Frame,FrameAllocator};
use memory::map::{TEMP_PAGE_A,TEMP_PAGE_B,RECURSIVE_ENTRY};
use multiboot2::BootInformation;

pub const PAGE_SIZE : usize = 4096;
const ENTRY_COUNT   : usize = 512;

pub type PhysicalAddress = usize;
pub type VirtualAddress  = usize;

#[derive(Clone)]
pub struct PageIter
{
    start : Page,
    end   : Page,
}

impl Iterator for PageIter
{
    type Item = Page;

    fn next(&mut self) -> Option<Page>
    {
        if self.start <= self.end
        {
            let page = self.start;
            self.start.number += 1;
            Some(page)
        }
        else
        {
            None
        }
    }
}

#[derive(Debug,Clone,Copy,PartialEq,Eq,PartialOrd,Ord)]
pub struct Page
{
    number : usize,
}

impl Add<usize> for Page
{
    type Output = Page;

    fn add(self, rhs : usize) -> Page
    {
        Page
        {
            number : self.number + rhs
        }
    }
}

impl Page
{
    pub fn range_inclusive(start : Page, end : Page) -> PageIter
    {
        PageIter
        {
            start : start,
            end   : end,
        }
    }

    pub fn get_start_address(&self) -> usize
    {
        self.number * PAGE_SIZE
    }

    pub fn get_containing_page(address : VirtualAddress) -> Page
    {
        assert!(address < 0x0000_8000_0000_0000 || address >= 0xffff_8000_0000_0000, "invalid address: 0x{:x}", address);
        Page { number : address / PAGE_SIZE }
    }
    
    fn p4_index(&self) -> usize { (self.number >> 27) & 0o777 }
    fn p3_index(&self) -> usize { (self.number >> 18) & 0o777 }
    fn p2_index(&self) -> usize { (self.number >>  9) & 0o777 }
    fn p1_index(&self) -> usize { (self.number >>  0) & 0o777 }
}

pub struct ActivePageTable
{
    mapper : Mapper,
}

impl Deref for ActivePageTable
{
    type Target = Mapper;

    fn deref(&self) -> &Mapper
    {
        &self.mapper
    }
}

impl DerefMut for ActivePageTable
{
    fn deref_mut(&mut self) -> &mut Mapper
    {
        &mut self.mapper
    }
}

impl ActivePageTable
{
    unsafe fn new() -> ActivePageTable
    {
        ActivePageTable { mapper : Mapper::new() }
    }

    /*
     * By returning a Mapper to the closure, instead of `self` (which is a ActivePageTable), we
     * stop it from calling this `with` method again, which fails because the recursive mapping
     * wouldn't be set up correctly.
     */
    pub fn with<F>(&mut self,
                   table : &mut InactivePageTable,
                   temporary_page : &mut temporary_page::TemporaryPage,
                   f : F
                  ) where F : FnOnce(&mut Mapper)
    {
        use x86_64::registers::control_regs;
        use x86_64::instructions::tlb;

        // Inner scope used to end the borrow of `temporary_page`
        {
            // Backup the current P4 and temporarily map it
            let original_p4 = Frame::get_containing_frame(control_regs::cr3().0 as usize);
            let p4_table = temporary_page.map_table_frame(original_p4.clone(), self);

            // Overwrite recursive mapping
            self.p4_mut()[RECURSIVE_ENTRY].set(table.p4_frame.clone(), EntryFlags::PRESENT |
                                                                       EntryFlags::WRITABLE);
            tlb::flush_all();

            // Execute in the new context
            f(self);

            // Restore recursive mapping to original P4
            p4_table[RECURSIVE_ENTRY].set(original_p4, EntryFlags::PRESENT | EntryFlags::WRITABLE);
            tlb::flush_all();
        }

        temporary_page.unmap(self);
    }

    /*
     * This switches to a new page table and returns the old (now inactive) one
     */
    pub fn switch(&mut self, new_table : InactivePageTable) -> InactivePageTable
    {
        use x86_64::PhysicalAddress;
        use x86_64::registers::control_regs;

        let old_table = InactivePageTable
                        {
                            p4_frame : Frame::get_containing_frame(control_regs::cr3().0 as usize)
                        };

        unsafe
        {
            /*
             * NOTE: We don't need to flush the TLB here because the CPU does it automatically when
             *       CR3 is reloaded.
             */
            control_regs::cr3_write(PhysicalAddress(new_table.p4_frame.get_start_address() as u64));
        }

        old_table
    }
}

pub struct InactivePageTable
{
    p4_frame : Frame
}

impl InactivePageTable
{
    pub fn new(frame : Frame, active_table : &mut ActivePageTable, temporary_page : &mut TemporaryPage) -> InactivePageTable
    {
        /*
         * We firstly temporarily map the page table into memory so we can zero it.
         * We then set up recursive mapping on the P4.
         *
         * NOTE: We use an inner scope here to make sure that `table` is dropped before
         *       we try to unmap the temporary page.
         */
        {
            let table = temporary_page.map_table_frame(frame.clone(), active_table);    // XXX: Causing crash
            table.zero();
            table[RECURSIVE_ENTRY].set(frame.clone(), EntryFlags::PRESENT | EntryFlags::WRITABLE);
        }

        temporary_page.unmap(active_table);
        InactivePageTable { p4_frame : frame }
    }
}

pub fn remap_kernel<A>(allocator : &mut A, boot_info : &BootInformation) -> ActivePageTable where A : FrameAllocator
{
    use memory::map::KERNEL_VMA;

    /*
     * First, we create a temporary page at an address that we know should be unused.
     */
    let mut temporary_page = TemporaryPage::new(Page::get_containing_page(TEMP_PAGE_A), allocator);
    println!("Created first temp page");
    let mut active_table = unsafe { ActivePageTable::new() };
    println!("Created new active table");
    let mut new_table = {
                            let frame = allocator.allocate_frame().expect("run out of frames");
                            InactivePageTable::new(frame, &mut active_table, &mut temporary_page)
                        };
    println!("Allocated new frame for table");

    /*
     * We must only map each page once, so we store the list without actually mapping them, resolve
     * duplicate page maps (when a page covers more than one structure), and then identity-map them
     * all.
     *
     * XXX: We don't have a heap at this point, so we create temporary page and use `TemporaryVec`.
     *      We store (the frame's flags, the frame itself, whether this is a duplicate entry).
     */
    let list_frame = allocator.allocate_frame().expect("run out of frames");
    let mut list_page = TemporaryPage::new(Page { number : TEMP_PAGE_B }, allocator);
    list_page.map(list_frame, &mut active_table);
    let mut frame_list : TemporaryVec<(EntryFlags,Frame,bool)> = unsafe { TemporaryVec::new(&mut list_page) };

    /*
     * TODO: Correctly map stuff here
     */
    active_table.with(&mut new_table, &mut temporary_page,
        |mapper| {
            let elf_sections_tag = boot_info.elf_sections_tag().expect("Memory map tag required");

            /*
             * Identity map all the sections of the kernel
             */
            for section in elf_sections_tag.sections()
            {
                // This section will not be allocated any memory, so skip it
                if !(section.is_allocated())
                {
                    continue;
                }

                assert!(section.start_address() % PAGE_SIZE == 0, "sections must be page aligned");

                let flags = EntryFlags::from_elf_section(section);
                let start_frame = Frame::get_containing_frame(section.start_address());
                let end_frame = Frame::get_containing_frame(section.end_address() - 1);

                for frame in Frame::range_inclusive(start_frame, end_frame)
                {
                    frame_list.push((flags, frame, true));
                }
            }

            // Identity-map the VGA buffer
            frame_list.push((EntryFlags::WRITABLE, Frame::get_containing_frame(0xb8000), true));

            // Identity-map any modules loaded by GRUB
            // TODO
/*            if boot_info.module_tags().count() > 0
            {
                for module_tag in boot_info.module_tags()
                {
                    let module_start = module_tag.start_address() as usize;
                    let module_end   = module_tag.end_address()   as usize;

                    for frame in Frame::range_inclusive(Frame::get_containing_frame(module_start),
                                                        Frame::get_containing_frame(module_end - 1))
                    {
                        frame_list.push((EntryFlags::PRESENT, frame, true));
                    }
                }
            }*/

            // Identity-map the Multiboot structure
            let multiboot_start = Frame::get_containing_frame(boot_info.start_address());
            let multiboot_end   = Frame::get_containing_frame(boot_info.end_address() - 1);

            for frame in Frame::range_inclusive(multiboot_start, multiboot_end)
            {
                frame_list.push((EntryFlags::PRESENT, frame, true));
            }

            // Coalesce duplicate frame mappings and identity-map each required frame
            for entry in frame_list.iter()
            {
                // If this frame has been marked a duplicate, skip it
                if !(entry.2)
                {
                    continue;
                }

                let mut flags : EntryFlags = entry.0;
                for duplicate in frame_list.iter().filter(
                    |e| {
                            e.1.get_start_address() == entry.1.get_start_address()
                        })
                {
                    /*
                     * This is a duplicate, but may have different permissions, so we merge them.
                     */
                    flags = flags.merge(duplicate.0);
                    duplicate.2 = false;
                }

                //mapper.identity_map(entry.1.clone(), entry.0, allocator);
            }
        });

    list_page.unmap(&mut active_table);
    let old_table = active_table.switch(new_table);

    // Turn the old P4 into a guard page for the stack
    let old_p4_page = Page::get_containing_page(old_table.p4_frame.get_start_address());
    active_table.unmap(old_p4_page, allocator);

    active_table
}
