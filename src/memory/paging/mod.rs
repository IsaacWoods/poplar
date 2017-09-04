/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

mod entry;
mod table;
mod temporary_page;
mod mapper;

pub use self::entry::*;
use self::mapper::Mapper;
use self::temporary_page::TemporaryPage;
use memory::{PAGE_SIZE,Frame,FrameAllocator};
use core::ops::{Deref,DerefMut};
use multiboot2::BootInformation;

const ENTRY_COUNT : usize = 512;

pub type PhysicalAddress = usize;
pub type VirtualAddress  = usize;

#[derive(Debug,Clone,Copy)]
pub struct Page
{
  number : usize,
}

impl Page
{
    fn get_start_address(&self) -> usize
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
            self.p4_mut()[511].set(table.p4_frame.clone(), PRESENT | WRITABLE);
            tlb::flush_all();

            // Execute in the new context
            f(self);

            // Restore recursive mapping to original P4
            p4_table[511].set(original_p4, PRESENT | WRITABLE);
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
            let table = temporary_page.map_table_frame(frame.clone(), active_table);
            table.zero();
            table[511].set(frame.clone(), PRESENT | WRITABLE);
        }

        temporary_page.unmap(active_table);
        InactivePageTable { p4_frame : frame }
    }
}

pub fn remap_kernel<A>(allocator : &mut A, boot_info : &BootInformation) where A : FrameAllocator
{
    /*
     * First, we create a temporary page at an address that we know should be unused.
     */
    let mut temporary_page = TemporaryPage::new(Page { number : 0xcafebabe }, allocator);
    let mut active_table = unsafe { ActivePageTable::new() };
    let mut new_table = {
                            let frame = allocator.allocate_frame().expect("run out of frames");
                            InactivePageTable::new(frame, &mut active_table, &mut temporary_page)
                        };

    active_table.with(&mut new_table, &mut temporary_page,
        |mapper| {
            let elf_sections_tag = boot_info.elf_sections_tag().expect("Memory map tag required");

            /*
             * Identity map all the sections of the kernel
             */
            for section in elf_sections_tag.sections()
            {
                if !(section.is_allocated())
                {
                    // The section is not in memory, so skip it
                    continue;
                }

                assert!(section.start_address() % PAGE_SIZE == 0, "sections must be page aligned");
                println!("mapping section at addr: {:#x}, size: {:#x}", section.addr, section.size);

                let flags = EntryFlags::from_elf_section(section);
                let start_frame = Frame::get_containing_frame(section.start_address());
                let end_frame = Frame::get_containing_frame(section.end_address() - 1);

                for frame in Frame::range_inclusive(start_frame, end_frame)
                {
                    mapper.identity_map(frame, flags, allocator);
                }
            }

            // Identity-map the VGA buffer
            let vga_buffer_frame = Frame::get_containing_frame(0xb8000);
            mapper.identity_map(vga_buffer_frame, WRITABLE, allocator);

            // Identity-map the Multiboot structure
            let multiboot_start = Frame::get_containing_frame(boot_info.start_address());
            let multiboot_end = Frame::get_containing_frame(boot_info.end_address() - 1);

            for frame in Frame::range_inclusive(multiboot_start, multiboot_end)
            {
                mapper.identity_map(frame, PRESENT, allocator);
            }
        });

    let old_table = active_table.switch(new_table);
}

pub fn test_paging<A>(allocator : &mut A) where A : FrameAllocator
{
    let mut page_table = unsafe { ActivePageTable::new() };
    
    // Test map_to
    let addr = 42*512*512*4096; // 42nd P3 entry
    let page = Page::get_containing_page(addr);
    let frame = allocator.allocate_frame().expect("run out of frames");
    println!("None = {:?}, map to {:?}", page_table.translate(addr), frame);
    page_table.map_to(page, frame, EntryFlags::empty(), allocator);
    println!("Some = {:?}", page_table.translate(addr));
    println!("next free frame: {:?}", allocator.allocate_frame());

    // Try to read stuff from the mapped test page
    println!("{:#x}", unsafe
                      {
                          *(Page::get_containing_page(addr).get_start_address() as *const u64)
                      });

    // Test unmap
    page_table.unmap(Page::get_containing_page(addr), allocator);
    println!("None = {:?}", page_table.translate(addr));

    // Should cause a PF
/*    println!("{:#x}", unsafe
                      {
                          *(Page::get_containing_page(addr).get_start_address() as *const u64)
                      });*/
}
