/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use alloc::Vec;
use xmas_elf::{ElfFile,program::Type};
use ::gdt::GdtSelectors;
use ::memory::{Frame,FrameAllocator,MemoryController};
use ::memory::paging::{Page,PhysicalAddress,VirtualAddress,InactivePageTable,ActivePageTable,
                       TemporaryPage,PAGE_SIZE};
use ::kernel::process::ProcessId;

pub enum ProcessState
{
    NotRunning(InactivePageTable),
    Running(ActivePageTable),
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
    image       : Image,
    // threads     : Vec<Thread>,
    thread      : Thread,
}

pub struct Thread
{
    instruction_pointer : VirtualAddress,
    stack_pointer       : VirtualAddress,
    base_pointer        : VirtualAddress,
}

impl Process
{
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
        let entry_point = VirtualAddress::new(elf.header.pt2.entry_point() as usize);

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
                mapper.p4[KERNEL_START_P4].set(kernel_p4_frame, EntryFlags::PRESENT |
                                                                EntryFlags::WRITABLE);

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
                                            let mut flags = EntryFlags::PRESENT |
                                                            EntryFlags::USER_ACCESSIBLE;

                                            if program_header.flags().is_write()
                                            {
                                                flags |= EntryFlags::WRITABLE;
                                            }

                                            if !program_header.flags().is_execute()
                                            {
                                                flags |= EntryFlags::NO_EXECUTE;
                                            }

                                            flags
                                        };

                            // TODO: This should remind us to do this properly when we hit BSS
                            // sections and stuff
                            assert!(program_header.file_size() == program_header.mem_size());

                            let num_pages = program_header.mem_size() as usize / PAGE_SIZE + 1;
                            for i in 0..num_pages
                            {
                                let offset = (i * PAGE_SIZE) as isize;
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
            state           : ProcessState::NotRunning(page_tables),
            image           : Image
                              {
                                  start : image_start,
                                  end   : image_end,
                              },
            // threads         : Vec::new(),
            thread          : Thread
                              {
                                  instruction_pointer   : entry_point,
                                  stack_pointer         : 0.into(),
                                  base_pointer          : 0.into(),
                              },
        }
    }

    pub unsafe fn switch_to<A>(&mut self, memory_controller : &mut MemoryController<A>)
        where A : FrameAllocator
    {
        use ::core::mem;

        /*
         * We want to replace the state, but can't move it out of the borrowed context normally,
         * so we have to do some unsafe magic. This also switches to the process' address space.
         */
         let old_state = mem::replace(&mut self.state, mem::uninitialized());

         let new_state = match old_state
                        {
                            ProcessState::NotRunning(inactive_table) =>
                            {
                                ProcessState::Running(memory_controller.kernel_page_table.switch(inactive_table))
                            },

                            ProcessState::Running(_) =>
                            {
                                panic!("Tried to switch to process that is already running!");
                            },
                        };

        let uninitialized = mem::replace(&mut self.state, new_state);
        mem::forget(uninitialized);
    }

    pub unsafe fn drop_to_usermode<A>(&mut self,
                                      gdt_selectors     : GdtSelectors,
                                      memory_controller : &mut MemoryController<A>) -> !
        where A : FrameAllocator
    {
        // Save the current kernel stack in the TSS
        let rsp : VirtualAddress;
        asm!("" : "={rsp}"(rsp) : : : "intel", "volatile");
        ::TSS.set_kernel_stack(rsp);

        // Switch to the process's address space
        self.switch_to(memory_controller);

        // Jump into ring3
        asm!("cli
              push r10      // Push selector for user data segment
              push r11      // Push new stack pointer
              push r12      // Push new RFLAGS
              push r13      // Push selector for user code segment
              push r14      // Push new instruction pointer

              xor rax, rax
              xor rbx, rbx
              xor rcx, rcx
              xor rdx, rdx
              xor rsi, rsi
              xor rdi, rdi
              xor r8, r8
              xor r9, r9
              xor r10, r10
              xor r11, r11
              xor r12, r12
              xor r13, r13
              xor r14, r14
              xor r15, r15

              iretq"
              :
              : "{r10}"(gdt_selectors.user_data.0),
                "{r11}"(self.thread.stack_pointer),
                "{rbp}"(self.thread.base_pointer),
                "{r12}"(1 << 9 | 1 << 2),   // We probably shouldn't leak flags out of kernel-space, 
                                            // so we set them to the bare minimum:
                                            //     * Bit 2 must be 1
                                            //     * Enable interrupts by setting bit 9
                "{r13}"(gdt_selectors.user_code.0),
                "{r14}"(self.thread.instruction_pointer)
              : // We technically don't clobber anything because this never returns
              : "intel", "volatile");
        unreachable!();
    }
}
