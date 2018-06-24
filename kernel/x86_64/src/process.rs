use alloc::{boxed::Box, Vec};
use core::{fmt, ptr};
use gdt::GdtSelectors;
use kernel::fs::File;
use kernel::node::Node;
use kernel::process::ProcessMessage;
use libmessage::NodeId;
use memory::paging::{
    ActivePageTable, EntryFlags, InactivePageTable, Page, VirtualAddress, PAGE_SIZE,
};
use memory::{Frame, MemoryController};
use xmas_elf::{
    program::{SegmentData, Type},
    ElfFile,
};

pub enum ProcessState {
    NotRunning(InactivePageTable),
    Running(ActivePageTable),
}

impl fmt::Debug for ProcessState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ProcessState::NotRunning(_) => write!(f, "Process is not running"),
            ProcessState::Running(_) => write!(f, "Process is running"),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ImageSegment {
    address: VirtualAddress, // This is the address that this segment should be mapped at
    start: Frame,
    end: Frame,
    flags: EntryFlags,
}

/// This is the image of a process, and represents the segments that should be loaded into memory
/// holding the code and data for a process. While instances of the same process can share
/// read-only segments in theory, each process must have its own `ProcessImage`.
// TODO: share read-only segments?
#[derive(Debug)]
pub struct ProcessImage {
    pub segments: Vec<ImageSegment>,
    pub entry_point: VirtualAddress,
}

impl ProcessImage {
    pub fn from_elf(image: &File, memory_controller: &mut MemoryController) -> ProcessImage {
        // TODO: proper error handling
        let elf =
            ElfFile::new(image.read().expect("Failed to read image")).expect("Failed to parse ELF");

        let mut segments = Vec::new();
        let entry_point = VirtualAddress::new(elf.header.pt2.entry_point() as usize);

        /*
         * Allocate memory for, and copy contents of, each segment in the ELF
         */
        for program_header in elf.program_iter() {
            match program_header.get_type().unwrap() {
                Type::Load => {
                    let virtual_address =
                        VirtualAddress::from(program_header.virtual_addr() as usize);
                    let flags = {
                        let mut flags = EntryFlags::PRESENT | EntryFlags::USER_ACCESSIBLE;

                        if program_header.flags().is_write() {
                            flags |= EntryFlags::WRITABLE;
                        }

                        if !program_header.flags().is_execute() {
                            flags |= EntryFlags::NO_EXECUTE;
                        }

                        flags
                    };

                    let needed_frames = Frame::needed_frames(program_header.mem_size() as usize);

                    let (segment_start, segment_end) = memory_controller
                        .frame_allocator
                        .allocate_frame_block(needed_frames)
                        .expect("Could not allocate frames for segment");

                    let segment_temp_mapping = memory_controller
                        .kernel_page_table
                        .map_physical_region::<u8>(
                            segment_start.start_address(),
                            segment_end.end_address(),
                            EntryFlags::PRESENT | EntryFlags::WRITABLE,
                            &mut memory_controller.frame_allocator,
                        );

                    // TODO: why doesn't ProgramHeader have a `raw_data` getter? Maybe we could
                    // upstream one?
                    if let SegmentData::Undefined(data) = program_header.get_data(&elf).unwrap() {
                        unsafe {
                            // TODO: if data.len() < mem_size, we should probably zero the rest of
                            // the segment
                            ptr::copy::<u8>(data.as_ptr(), segment_temp_mapping.ptr, data.len());
                        }
                    } else {
                        panic!("xmas-elf should always give us a SegmentData::Undefined for a LOAD segment!");
                    }

                    segments.push(ImageSegment {
                        address: virtual_address,
                        start: segment_start,
                        end: segment_end,
                        flags,
                    });

                    // TODO: get working: some issue with how the heap tries to free the memory
                    // used - move away from using the heap for physical mappings?
                    // memory_controller.kernel_page_table.unmap_physical_region(segment_temp_mapping,
                    //                                                           &mut memory_controller.frame_allocator);
                }

                typ => {
                    error!("Unsupported program header type in parsed ELF: {:?}", typ);
                }
            }
        }

        ProcessImage {
            segments,
            entry_point,
        }
    }
}

#[derive(Debug)]
pub struct Process {
    state: ProcessState,
    // threads     : Vec<Thread>,
    thread: Thread,
}

#[derive(Debug)]
pub struct Thread {
    stack_top: VirtualAddress,
    stack_size: usize,

    instruction_pointer: VirtualAddress,
    stack_pointer: VirtualAddress,
    base_pointer: VirtualAddress,
}

impl Process {
    pub fn new(image: ProcessImage, memory_controller: &mut MemoryController) -> Process {
        use memory::map::KERNEL_START_P4;

        const STACK_BOTTOM: VirtualAddress = VirtualAddress::new(0x1000_0000); // TODO: decide actual address
        const INITIAL_STACK_SIZE: usize = 2 * PAGE_SIZE;
        const STACK_TOP: VirtualAddress = STACK_BOTTOM.offset((INITIAL_STACK_SIZE - 1) as isize);

        let entry_point = image.entry_point;

        // Create the process' page tables
        let mut page_tables = {
            let frame = memory_controller.frame_allocator.allocate_frame().unwrap();
            InactivePageTable::new(
                frame,
                &mut memory_controller.kernel_page_table,
                &mut memory_controller.frame_allocator,
            )
        };

        /*
         * This is the frame holding the kernel's P3 - we copy its address into the 511th entry of
         * every process' P4 to keep the kernel mapped.
         */
        let kernel_p3_frame = memory_controller.kernel_page_table.p4[KERNEL_START_P4]
            .pointed_frame()
            .expect("Could not find kernel P3 frame");

        let kernel_table = &mut memory_controller.kernel_page_table;

        kernel_table.with(
            &mut page_tables,
            &mut memory_controller.frame_allocator,
            |mapper, allocator| {
                /*
                 * We map the entire kernel into each user-mode process. Instead of cloning the
                 * entire thing, we just steal the frame from the kernel's P4.
                 */
                mapper.p4[KERNEL_START_P4]
                    .set(kernel_p3_frame, EntryFlags::PRESENT | EntryFlags::WRITABLE);

                // Map in each segment from the image
                for segment in image.segments {
                    info!(
                        "Mapping segment starting {:#x} into process address space",
                        segment.address
                    );
                    for (i, frame) in Frame::range_inclusive(segment.start, segment.end).enumerate()
                    {
                        let page_address = segment.address.offset((i * PAGE_SIZE) as isize);
                        assert!(page_address.is_page_aligned());

                        mapper.map_to(
                            Page::containing_page(page_address),
                            frame,
                            segment.flags,
                            allocator,
                        );
                    }
                }

                // Allocate a stack for the main thread
                for stack_page in Page::range_inclusive(
                    Page::containing_page(STACK_BOTTOM),
                    Page::containing_page(STACK_TOP),
                ) {
                    mapper.map(
                        stack_page,
                        EntryFlags::PRESENT | EntryFlags::USER_ACCESSIBLE | EntryFlags::WRITABLE | EntryFlags::NO_EXECUTE,
                        allocator,
                    );
                }

                // Allocate and map the Send Buffer
                mapper.map(Page::containing_page(::libmessage::process::SEND_BUFFER_ADDRESS.into()),
                           EntryFlags::PRESENT | EntryFlags::USER_ACCESSIBLE | EntryFlags::WRITABLE | EntryFlags::NO_EXECUTE,
                           allocator);

                // TODO: Map stuff for the new process
                //          * The ELF sections - makes up the image
                //          * A stack
                //          * In the future, any priviledged memory requests we want to grant
            },
        );

        // TODO: get unmapping this working
        // memory_controller.kernel_page_table.unmap_physical_region(elf_temp_mapping, &mut memory_controller.frame_allocator);

        Process {
            state: ProcessState::NotRunning(page_tables),
            // threads         : Vec::new(),
            thread: Thread {
                stack_top: STACK_TOP,
                stack_size: INITIAL_STACK_SIZE,

                instruction_pointer: entry_point,
                stack_pointer: STACK_TOP,
                base_pointer: STACK_TOP,
            },
        }
    }

    pub unsafe fn switch_to(&mut self, memory_controller: &mut MemoryController) {
        use core::mem;

        /*
         * We want to replace the state, but can't move it out of the borrowed context normally,
         * so we have to do some unsafe magic. This also switches to the process' address space.
         */
        let old_state = mem::replace(&mut self.state, mem::uninitialized());

        let new_state = match old_state {
            ProcessState::NotRunning(inactive_table) => {
                ProcessState::Running(memory_controller.kernel_page_table.switch(inactive_table))
            }

            ProcessState::Running(_) => {
                panic!("Tried to switch to process that is already running!");
            }
        };

        let uninitialized = mem::replace(&mut self.state, new_state);
        mem::forget(uninitialized);
    }

    pub unsafe fn drop_to_usermode(
        &mut self,
        gdt_selectors: &GdtSelectors,
        memory_controller: &mut MemoryController,
    ) -> ! {
        // Save the current kernel stack in the TSS
        let rsp: VirtualAddress;
        asm!("" : "={rsp}"(rsp) : : : "intel", "volatile");
        ::PLATFORM.tss.set_kernel_stack(rsp);

        // Switch to the process's address space
        self.switch_to(memory_controller);

        // TEMP: zero the Send Buffer for Isaac's sanity
        ::core::intrinsics::volatile_set_memory(0xff_0000_0000 as *mut u8,
                                                0x00,
                                                4096);

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

impl Node for Process {
    type MessageType = ProcessMessage;

    fn message(&mut self, sender: NodeId, message: ProcessMessage) -> Result<(), ()> {
        match message {
            ProcessMessage::DropToUsermode => {
                use PLATFORM;
                info!("Dropping to usermode!");
                unsafe {
                    self.drop_to_usermode(
                        PLATFORM.gdt_selectors.as_ref().unwrap(),
                        PLATFORM.memory_controller.as_mut().unwrap(),
                    );
                }
            }
        }
    }
}
