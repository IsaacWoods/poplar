/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

mod entry;
mod table;

pub use self::entry::*;
use memory::{PAGE_SIZE,Frame,FrameAllocator};
use self::table::{Table,Level4};
use core::ptr::Unique;

const ENTRY_COUNT : usize = 512;

pub type PhysicalAddress = usize;
pub type VirtualAddress  = usize;

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
    p4 : Unique<Table<Level4>>
}

impl ActivePageTable
{
    pub unsafe fn new() -> ActivePageTable
    {
        ActivePageTable { p4 : Unique::new_unchecked(table::P4) }
    }

    fn p4(&self) -> &Table<Level4>
    {
        unsafe { self.p4.as_ref() }
    }

    fn p4_mut(&mut self) -> &mut Table<Level4>
    {
        unsafe { self.p4.as_mut() }
    }

    pub fn translate(&self, virtual_address : VirtualAddress) -> Option<PhysicalAddress>
    {
        let offset = virtual_address % PAGE_SIZE;
        self.translate_page(Page::get_containing_page(virtual_address)).map(|frame| frame.number * PAGE_SIZE + offset)
    }

    fn translate_page(&self, page : Page) -> Option<Frame>
    {
        use self::entry::HUGE_PAGE;
        let p3 = self.p4().next_table(page.p4_index());

        let huge_page =
            || {
                p3.and_then(
                    |p3| {
                        let p3_entry = &p3[page.p3_index()];
                        // 1GiB page?
                        if let Some(start_frame) = p3_entry.get_pointed_frame()
                        {
                            if p3_entry.flags().contains(HUGE_PAGE)
                            {
                                assert!(start_frame.number % (ENTRY_COUNT * ENTRY_COUNT) == 0);
                                return Some(Frame
                                            {
                                                number : start_frame.number + page.p2_index() * ENTRY_COUNT + page.p1_index()
                                            });
                            }
                        }

                        if let Some(p2) = p3.next_table(page.p3_index())
                        {
                            let p2_entry = &p2[page.p2_index()];
                            // 2MiB page?
                            if let Some(start_frame) = p2_entry.get_pointed_frame()
                            {
                                if p2_entry.flags().contains(HUGE_PAGE)
                                {
                                    // address must be 2MiB aligned
                                    assert!(start_frame.number % ENTRY_COUNT == 0);
                                    return Some(Frame { number : start_frame.number + page.p1_index() });
                                }
                            }
                        }
                        None
                    })
            };
    
        p3.and_then(|p3| p3.next_table(page.p3_index()))
          .and_then(|p2| p2.next_table(page.p2_index()))
          .and_then(|p1| p1[page.p1_index()].get_pointed_frame())
          .or_else(huge_page)
    }

    pub fn map<A>(&mut self, page : Page, flags : EntryFlags, allocator : &mut A) where A : FrameAllocator
    {
        let frame = allocator.allocate_frame().expect("out of memory");
        self.map_to(page, frame, flags, allocator)
    }

    pub fn identity_map<A>(&mut self, frame : Frame, flags : EntryFlags, allocator : &mut A) where A : FrameAllocator
    {
        let page = Page::get_containing_page(frame.get_start_address());
        self.map_to(page, frame, flags, allocator);
    }

    pub fn unmap<A>(&mut self, page : Page, allocator : &mut A) where A : FrameAllocator
    {
        assert!(self.translate(page.get_start_address()).is_some());

        let p1 = self.p4_mut()
                     .next_table_mut(page.p4_index())
                     .and_then(|p3| p3.next_table_mut(page.p3_index()))
                     .and_then(|p2| p2.next_table_mut(page.p2_index()))
                     .expect("we don't support huge pages");
        let frame = p1[page.p1_index()].get_pointed_frame().unwrap();
        p1[page.p1_index()].set_unused();
    
        // Clear the TLB entry for this page
        use x86_64::instructions::tlb;
        use x86_64::VirtualAddress;
        tlb::flush(VirtualAddress(page.get_start_address()));

        // TODO free p(1,2,3) table if it has become empty
        allocator.deallocate_frame(frame);
    }

    pub fn map_to<A>(&mut self, page : Page, frame : Frame, flags : EntryFlags, allocator : &mut A) where A : FrameAllocator
    {
        let p4 = self.p4_mut();
        let mut p3 = p4.next_table_create(page.p4_index(), allocator);
        let mut p2 = p3.next_table_create(page.p3_index(), allocator);
        let mut p1 = p2.next_table_create(page.p2_index(), allocator);
    
        assert!(p1[page.p1_index()].is_unused());
        p1[page.p1_index()].set(frame, flags | PRESENT);
    }
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
