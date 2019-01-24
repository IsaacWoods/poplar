use super::{memory::userspace_map::*, Arch};
use alloc::vec::Vec;
use log::info;
use x86_64::memory::{
    kernel_map::KERNEL_P4_ENTRY,
    paging::{
        entry::EntryFlags,
        table::RecursiveMapping,
        ActivePageTable,
        InactivePageTable,
        Page,
    },
    VirtualAddress,
};

pub enum ProcessState {
    NotRunning(InactivePageTable<RecursiveMapping>),
    Running(ActivePageTable<RecursiveMapping>),
}

/// Represents a thread, including its stack and the contents of RIP, RSP, and RBP. The rest of the
/// registers are pushed to the stack when a context switch occurs.
pub struct Thread {
    pub id: u8,

    pub stack_top: VirtualAddress,
    pub stack_size: usize,

    pub instruction_pointer: VirtualAddress,
    pub stack_pointer: VirtualAddress,
    pub base_pointer: VirtualAddress,
}

pub struct Process {
    state: ProcessState,
    threads: Vec<Thread>,
}

impl Process {
    pub fn new(
        arch: &Arch,
        mut page_table: InactivePageTable<RecursiveMapping>,
        entry_point: VirtualAddress,
    ) -> Process {
        /*
         * NOTE: safe to unwrap because we wouldn't be able to fetch these instructions if the
         * kernel wasn't mapped.
         */
        let kernel_p3_frame =
            arch.kernel_page_table.lock().p4[KERNEL_P4_ENTRY].pointed_frame().unwrap();

        /*
         * Because the main thread is id 0, its stacks are at the beginning of the relevant
         * areas.
         */
        let stack_bottom = USER_STACKS_START;
        let stack_top = (stack_bottom + INITIAL_STACK_SIZE).unwrap();

        arch.kernel_page_table.lock().with(
            &mut page_table,
            &arch.physical_memory_manager,
            |mapper, allocator| {
                /*
                 * We map the kernel into every process' address space by stealing the address of
                 * the kernel's P3, and putting it into the process' P4.
                 */
                mapper.p4[KERNEL_P4_ENTRY]
                    .set(kernel_p3_frame, EntryFlags::PRESENT | EntryFlags::WRITABLE);

                /*
                 * Map the main thread's stack.
                 */
                mapper.map_range(
                    Page::contains(stack_bottom)..Page::contains(stack_top),
                    EntryFlags::PRESENT
                        | EntryFlags::WRITABLE
                        | EntryFlags::NO_EXECUTE
                        | EntryFlags::USER_ACCESSIBLE,
                    allocator,
                );
            },
        );

        let main_thread = Thread {
            id: 0,
            stack_top,
            stack_size: INITIAL_STACK_SIZE,
            instruction_pointer: entry_point,
            stack_pointer: stack_top,
            base_pointer: stack_top,
        };

        Process { state: ProcessState::NotRunning(page_table), threads: vec![main_thread] }
    }
}

/// Drop to Ring 3, into a process. This is used for the initial transition from kernel to user
/// mode after the CPU has been brought up.
pub fn drop_to_usermode(arch: &Arch, process: &mut Process) -> ! {
    // TODO
    unimplemented!();
}
