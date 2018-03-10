/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use alloc::Vec;
use goblin::{elf::Elf};
use ::memory::{MemoryController,FrameAllocator};
use ::memory::paging::{InactivePageTable,TemporaryPage,PhysicalAddress};
use ::kernel::process::ProcessId;

pub enum ProcessState
{
    NotRunning,
    Running,
}

pub struct Image
{
    start   : PhysicalAddress,
    end     : PhysicalAddress,
}

pub struct Process
{
    id          : ProcessId,
    state       : ProcessState,
    page_tables : InactivePageTable,
    image       : Image,
    threads     : Vec<Thread>,
}

pub struct Thread
{
    // TODO: Store stack pointer and stuff here
}

impl Process
{
    // TODO: pass an ELF or something to parse
    pub fn new<A>(id                : ProcessId,
                  image_start       : PhysicalAddress,
                  image_end         : PhysicalAddress,
                  memory_controller : &mut MemoryController<A>) -> Process
        where A : FrameAllocator
    {
        use ::memory::paging::EntryFlags;
        use ::memory::map::KERNEL_START_P4;

        let mut temporary_page = TemporaryPage::new(::memory::map::TEMP_PAGE,
                                                    &mut memory_controller.frame_allocator);
        let temporary_frame = memory_controller.frame_allocator.allocate_frame().unwrap();
        temporary_page.map(temporary_frame, &mut memory_controller.kernel_page_table);

        // Create the process' page tables
        let mut page_tables =
            {
                let frame = memory_controller.frame_allocator.allocate_frame().unwrap();
                InactivePageTable::new(frame,
                                       &mut memory_controller.kernel_page_table,
                                       &mut temporary_page)
            };

        let kernel_p4_frame = memory_controller.kernel_page_table.p4[KERNEL_START_P4].pointed_frame().expect("Could not find kernel P4 frame");

        memory_controller.kernel_page_table.with(&mut page_tables, &mut temporary_page,
            |mapper| {

                /*
                 * We map the entire kernel into each user-mode process. Instead of cloning the
                 * entire thing, we just steal the frame from the kernel's P4.
                 */
                mapper.p4[KERNEL_START_P4].set(kernel_p4_frame, EntryFlags::PRESENT);

                /*
                 * Map the image.
                 */
                // TODO

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
            image           : Image
                              {
                                  start : image_start,
                                  end   : image_end,
                              },
            threads         : Vec::new(),
        }
    }

    pub unsafe fn switch_to<A>(&mut self, memory_controller : &mut MemoryController<A>)
        where A : FrameAllocator
    {
        // TODO: do the switch
        //let old_table = memory_controller.active_table.switch(self.page_tables);    // TODO: idk
    }
}
