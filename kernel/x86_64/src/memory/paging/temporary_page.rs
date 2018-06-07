use memory::paging::entry::EntryFlags;
use memory::paging::table::{Level1, Table};
use memory::paging::{ActivePageTable, Page, VirtualAddress};
use memory::{Frame, FrameAllocator};

pub struct TemporaryPage {
    page: Page,
}

impl TemporaryPage {
    pub fn new(page: Page) -> TemporaryPage {
        TemporaryPage { page }
    }

    /// Map this temporary page into the given frame in the active page table. Return the start
    /// address of the page.
    pub fn map(
        &mut self,
        frame: Frame,
        active_table: &mut ActivePageTable,
        frame_allocator: &mut FrameAllocator,
    ) -> VirtualAddress {
        assert!(
            active_table.translate_page(self.page).is_none(),
            "Temp page is already mapped"
        );
        active_table.map_to(self.page, frame, EntryFlags::WRITABLE, frame_allocator);
        self.page.start_address()
    }

    /// Maps a given frame into memory and returns it as a P1.
    /// Used to temporarily map page tables into memory. We return a Level1 table so next_table()
    /// can't be called, becuase this temporary page won't be part of the recursive structure
    pub fn map_table_frame(
        &mut self,
        frame: Frame,
        active_table: &mut ActivePageTable,
        frame_allocator: &mut FrameAllocator,
    ) -> &mut Table<Level1> {
        unsafe {
            &mut *(self.map(frame, active_table, frame_allocator).mut_ptr() as *mut Table<Level1>)
        }
    }

    pub fn unmap(
        &mut self,
        active_table: &mut ActivePageTable,
        frame_allocator: &mut FrameAllocator,
    ) {
        active_table.unmap(self.page, frame_allocator)
    }
}
