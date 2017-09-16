/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use super::{Page,ActivePageTable,VirtualAddress};
use super::table::{Table,Level1};
use memory::{Frame,FrameAllocator};

pub struct TemporaryPage
{
    page : Page,
    allocator : TinyAllocator,
}

impl TemporaryPage
{
    pub fn new<A>(page : Page, allocator : &mut A) -> TemporaryPage where A : FrameAllocator
    {
        TemporaryPage
        {
            page : page,
            allocator : TinyAllocator::new(allocator)
        }
    }

    /*
     * Map this temporary page into the given frame in the active page table. Return the start
     * address of the page.
     */
    pub fn map(&mut self, frame : Frame, active_table : &mut ActivePageTable) -> VirtualAddress
    {
        use super::entry::WRITABLE;
        assert!(active_table.translate_page(self.page).is_none(), "temp page is already mapped");
        active_table.map_to(self.page, frame, WRITABLE, &mut self.allocator);
        self.page.get_start_address()
    }

    /*
     * Maps a given frame into memory and returns it as a P1.
     * Used to temporarily map page tables into memory.
     *
     * NOTE: We return a Level1 table so next_table() can't be called,
     *       becuase this temporary page won't be part of the recursive
     *       structure
     */
    pub fn map_table_frame(&mut self, frame : Frame, active_table : &mut ActivePageTable) -> &mut Table<Level1>
    {
        unsafe { &mut *(self.map(frame, active_table) as *mut Table<Level1>) }
    }

    pub fn unmap(&mut self, active_table : &mut ActivePageTable)
    {
        active_table.unmap(self.page, &mut self.allocator)
    }

    pub fn get_start_address(&self) -> usize
    {
        self.page.get_start_address()
    }
}

/*
 * TinyAllocator is an allocator that can only hold 3 frames. It is only useful when temporarily
 * mapping pages, to store a single set of page table pages (one P3, one P2 and one P1).
 */
struct TinyAllocator([Option<Frame>; 3]);

impl TinyAllocator
{
    fn new<A>(allocator : &mut A) -> TinyAllocator where A : FrameAllocator
    {
        let mut f = || allocator.allocate_frame();
        let frames = [f(), f(), f()];
        TinyAllocator(frames)
    }
}

impl FrameAllocator for TinyAllocator
{
    fn allocate_frame(&mut self) -> Option<Frame>
    {
        for frame_option in &mut self.0
        {
            if frame_option.is_some()
            {
                return frame_option.take();
            }
        }
        None
    }

    fn deallocate_frame(&mut self, frame : Frame)
    {
        for frame_option in &mut self.0
        {
            if frame_option.is_none()
            {
                *frame_option = Some(frame);
                return;
            }
        }
        panic!("Tiny allocator can only hold 3 frames");
    }
}
