/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use ::memory::{MemoryController,FrameAllocator};
use ::memory::paging::{InactivePageTable,TemporaryPage,PhysicalAddress};
use ::kernel::process::ProcessId;

pub enum ProcessState
{
    NotRunning,
    Running,
}

pub struct Process
{
    id          : ProcessId,
    state       : ProcessState,
    page_tables : InactivePageTable,
    image_start : PhysicalAddress,
    image_end   : PhysicalAddress,
}

impl Process
{
    // TODO: pass an ELF or something to parse
    pub fn new<A>(id                : ProcessId,
                  memory_controller : &mut MemoryController<A>) -> Process
        where A : FrameAllocator
    {
        let mut temporary_page = TemporaryPage::new(::memory::map::TEMP_PAGE,
                                                    &mut memory_controller.frame_allocator);
        let temporary_frame = memory_controller.frame_allocator.allocate_frame().unwrap();
        temporary_page.map(temporary_frame, &mut memory_controller.kernel_page_table);

        let mut page_tables =
            {
                let frame = memory_controller.frame_allocator.allocate_frame().unwrap();
                InactivePageTable::new(frame,
                                       &mut memory_controller.kernel_page_table,
                                       &mut temporary_page)
            };

        memory_controller.kernel_page_table.with(&mut page_tables, &mut temporary_page,
            |mapper| {
                // TODO: Map stuff for the new process
                //          * The ELF sections - makes up the image
                //          * A stack
                //          * In the future, any priviledged memory requests we want to grant
            });

        temporary_page.unmap(&mut memory_controller.kernel_page_table);
//        memory_controller.frame_allocator.deallocate_frame(temporary_frame);

        Process
        {
            id,
            page_tables,
            state           : ProcessState::NotRunning,
            image_start     : 0.into(),
            image_end       : 0.into(),
        }
    }

    pub unsafe fn switch_to<A>(&mut self, memory_controller : &mut MemoryController<A>)
        where A : FrameAllocator
    {
        // TODO: do the switch
        //let old_table = memory_controller.active_table.switch(self.page_tables);    // TODO: idk
    }
}
