/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use alloc::Vec;
use xmas_elf::{ElfFile,program::ProgramHeader,program::Type};
use ::memory::{Frame,FrameAllocator,MemoryController};
use ::memory::paging::{Page,PhysicalAddress,VirtualAddress,InactivePageTable,TemporaryPage,PAGE_SIZE};
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
        use ::memory::paging::{EntryFlags,PhysicalMapping,ActivePageTable};
        use ::memory::map::KERNEL_START_P4;

        let mut temporary_page = TemporaryPage::new(::memory::map::TEMP_PAGE,
                                                    &mut memory_controller.frame_allocator);

        let elf_temp_mapping : PhysicalMapping<u8> = memory_controller.kernel_page_table.map_physical_region(image_start,
                                                                                       image_end,
                                                                                       EntryFlags::PRESENT,
                                                                                       &mut memory_controller.frame_allocator);
        let elf = ElfFile::new(unsafe { ::core::slice::from_raw_parts(elf_temp_mapping.ptr, elf_temp_mapping.size) }).unwrap();

        // Create the process' page tables
        let mut page_tables =
            {
                let frame = memory_controller.frame_allocator.allocate_frame().unwrap();
                InactivePageTable::new(frame,
                                       &mut memory_controller.kernel_page_table,
                                       &mut temporary_page)
            };

        let kernel_p4_frame = memory_controller.kernel_page_table.p4[KERNEL_START_P4].pointed_frame().expect("Could not find kernel P4 frame");

        /*
         * We can't borrow the real ActivePageTable because then we can't allocate in the closure.
         * This should do as good a job.
         */
        let mut kernel_table = unsafe { ActivePageTable::new() };

        kernel_table.with(&mut page_tables, &mut temporary_page,
            |mapper| {
                /*
                 * We map the entire kernel into each user-mode process. Instead of cloning the
                 * entire thing, we just steal the frame from the kernel's P4.
                 */
                mapper.p4[KERNEL_START_P4].set(kernel_p4_frame, EntryFlags::PRESENT);

                /*
                 * Map the image.
                 */
                for program_header in elf.program_iter()
                {
                    match program_header.get_type().unwrap()
                    {
                        Type::Null => {},

                        Type::Load =>
                        {
                            let physical_address = image_start.offset(program_header.offset() as isize);
                            let flags = {
                                            let mut flags = EntryFlags::PRESENT | EntryFlags::USER_ACCESSIBLE;
                                            if program_header.flags().is_write()    { flags |= EntryFlags::WRITABLE;    }
                                            if !program_header.flags().is_execute() { flags |= EntryFlags::NO_EXECUTE;  }
                                            flags
                                        };

                            assert!(program_header.file_size() == program_header.mem_size());

                            let num_pages = program_header.mem_size() as usize / PAGE_SIZE + 1;
                            for i in (0..num_pages)
                            {
                                let offset = (i * PAGE_SIZE) * isize;
                                let frame_address = physical_address.offset(offset);
                                let page_address = VirtualAddress::new(program_header.virtual_addr() as usize).offset(offset);
                                mapper.map_to(Page::containing_page(page_address),
                                              Frame::containing_frame(frame_address),
                                              flags,
                                              &mut memory_controller.frame_allocator);
                            }
                        },

                        _ =>
                        {
                            error!("Unsupported program header type in parsed ELF!");
                        },
                    }
                }
                
                // TODO: Map stuff for the new process
                //          * The ELF sections - makes up the image
                //          * A stack
                //          * In the future, any priviledged memory requests we want to grant
            });

        // TODO: get unmapping this working
        // memory_controller.kernel_page_table.unmap_physical_region(elf_temp_mapping, &mut memory_controller.frame_allocator);

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
