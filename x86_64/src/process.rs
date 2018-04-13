/*
 * Copyright (C) 2017, Pebble Developers.
 * See LICENCE.md
 */

use core::{fmt,ptr};
use xmas_elf::{ElfFile,program::{Type,ProgramHeader}};
use gdt::GdtSelectors;
use memory::{Frame,MemoryController,FrameAllocator};
use memory::paging::{Page,PhysicalAddress,VirtualAddress,InactivePageTable,ActivePageTable,PAGE_SIZE,
                     EntryFlags,Mapper};
use kernel::node::Node;
use kernel::process::ProcessMessage;
use libpebble::node::NodeId;

pub enum ProcessState
{
    NotRunning(InactivePageTable),
    Running(ActivePageTable),
}

impl fmt::Debug for ProcessState
{
    fn fmt(&self, f : &mut fmt::Formatter) -> fmt::Result
    {
        match *self
        {
            ProcessState::NotRunning(_) => write!(f, "Process is not running"),
            ProcessState::Running(_)    => write!(f, "Process is running"),
        }
    }
}

#[derive(Debug)]
pub struct Image
{
    start   : PhysicalAddress,
    end     : PhysicalAddress,
}

#[derive(Debug)]
pub struct Process
{
    state       : ProcessState,
    image       : Image,
    // threads     : Vec<Thread>,
    thread      : Thread,
}

#[derive(Debug)]
pub struct Thread
{
    instruction_pointer : VirtualAddress,
    stack_pointer       : VirtualAddress,
    base_pointer        : VirtualAddress,
}

impl Process
{
    pub fn new(image_start          : PhysicalAddress,
               image_end            : PhysicalAddress,
               memory_controller    : &mut MemoryController) -> Process
    {
        use ::memory::map::KERNEL_START_P4;

        info!("Creating process with image between {:#x} and {:#x}", image_start, image_end);
        let elf_temp_mapping = memory_controller.kernel_page_table
                                                .map_physical_region(image_start,
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
                                       &mut memory_controller.frame_allocator)
            };

        /*
         * This is the frame holding the kernel's P3 - we copy its address into the 511th entry of
         * every process' P4 to keep the kernel mapped.
         */
        let kernel_p3_frame = memory_controller.kernel_page_table
                                               .p4[KERNEL_START_P4]
                                               .pointed_frame()
                                               .expect("Could not find kernel P3 frame");

        let kernel_table = &mut memory_controller.kernel_page_table;

        kernel_table.with(&mut page_tables, &mut memory_controller.frame_allocator,
            |mapper, allocator| {
                /*
                 * We map the entire kernel into each user-mode process. Instead of cloning the
                 * entire thing, we just steal the frame from the kernel's P4.
                 */
                mapper.p4[KERNEL_START_P4].set(kernel_p3_frame, EntryFlags::PRESENT |
                                                                EntryFlags::WRITABLE);

                /*
                 * Map the image.
                 */
                for program_header in elf.program_iter()
                {
                    match program_header.get_type().unwrap()
                    {
                        Type::Load =>
                        {
                            info!("Mapping LOAD segment for process");
                            let image_segment_start = VirtualAddress::from(elf_temp_mapping.ptr).offset(program_header.offset() as isize);
                            // map_load_segment(image_segment_start, &program_header, mapper, allocator);

                            info!("Testing mapping stuff and things. Mapping address 0x400000");
                            const ADDRESS : VirtualAddress = VirtualAddress::new(0x6000);

                            let page = Page::containing_page(ADDRESS);
                            info!("Containing page starts at {:#x}", page.start_address());
                            let frame = allocator.allocate_frame().expect("Oopsie poopsie");
                            mapper.map_to(page, frame, EntryFlags::PRESENT | EntryFlags::WRITABLE, allocator);
                            info!("Page mapped. Reading from page");

                            ::tlb::flush();

                            unsafe
                            {
                                info!("Read from page: {}", ptr::read::<u8>(ADDRESS.ptr()));
                            }
                            info!("Paging test complete");
                        },

                        typ =>
                        {
                            error!("Unsupported program header type in parsed ELF: {:?}", typ);
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

    pub unsafe fn switch_to(&mut self, memory_controller : &mut MemoryController)
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

    pub unsafe fn drop_to_usermode(&mut self,
                                   gdt_selectors        : &GdtSelectors,
                                   memory_controller    : &mut MemoryController) -> !
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

impl Node for Process
{
    type MessageType = ProcessMessage;

    fn message(&mut self, sender : NodeId, message : ProcessMessage)
    {
        // TODO: what should we do here? We somehow need to signal to the process that it's
        // recieved a message, which is gonna be handled in userspace. We could reserve some memory
        // as a sort-of queue to put unhandled messages in, and then expect the process to empty it
        // as it pleases. What happens when we run out of space in this queue (we could expand it,
        // up to a threshold, then terminate the process?) Or just terminate it right away for
        // simplicity?
        //
        // Do we also want to map the process address space for each message, or keep it in a
        // kernel-space queue for a while and map it when we do a context switch into that process?
        // That seems like a better design.
        match message
        {
            ProcessMessage::DropToUsermode =>
            {
                use ::PLATFORM;

                info!("Dropping to usermode in process!");
                unsafe
                {
                    self.drop_to_usermode(PLATFORM.gdt_selectors.as_ref().unwrap(),
                                          PLATFORM.memory_controller.as_mut().unwrap());
                }
            },
        }
    }
}

fn map_load_segment(image_segment_start : VirtualAddress,
                    segment             : &ProgramHeader,
                    mapper              : &mut Mapper,
                    allocator           : &mut FrameAllocator)
{
    panic!("Remove this panic");
    let flags = {
                    let mut flags = EntryFlags::PRESENT |
                                    EntryFlags::USER_ACCESSIBLE;

                    if segment.flags().is_write()
                    {
                        flags |= EntryFlags::WRITABLE;
                    }

                    if !segment.flags().is_execute()
                    {
                        flags |= EntryFlags::NO_EXECUTE;
                    }

                    flags
                };

    // TODO: This should remind us to do this properly when we hit BSS
    // sections and stuff
    assert!(segment.file_size() == segment.mem_size());

    /*
     * The segment may not be frame-aligned and so will not map correctly
     * onto its virtual address, so we remap it onto a frame boundary.
     *
     * TODO: if we create multiple instances of one process, we'd be
     * keeping multiple copies of the same image. We should sort-of cache
     * images, which are then referenced by their processes
     */
    // TODO: replace this with a map_page_range function or something?
    let virtual_address = VirtualAddress::new(segment.virtual_addr() as usize);
    info!("LOAD virtual address {:#x}", virtual_address);
    let num_pages = segment.mem_size() as usize / PAGE_SIZE;
    for page in Page::range_inclusive(Page::containing_page(virtual_address),
                                      Page::containing_page(virtual_address.offset(segment.mem_size() as isize)))
    {
        // Map the page
        info!("Page start address = {:#x}", page.start_address());
        info!("Mapping page for process {:?}", page);
        mapper.map(page, flags, allocator);
    }

    info!("First page mapped to {:?}", mapper.translate(virtual_address));

    // Copy the data into the image
    unsafe
    {
        info!("Read from \"mapped\" page: {}", ptr::read::<u8>(virtual_address.mut_ptr()));
        // ptr::copy::<u8>(image_segment_start.ptr(), virtual_address.mut_ptr(), segment.file_size() as usize);
    }
}
