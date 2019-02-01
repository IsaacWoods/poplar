use super::{memory::userspace_map::*, Arch};
use alloc::vec::Vec;
use log::info;
use x86_64::{
    hw::tss::Tss,
    memory::{
        kernel_map::KERNEL_P4_ENTRY,
        paging::{
            entry::EntryFlags,
            table::RecursiveMapping,
            ActivePageTable,
            InactivePageTable,
            Page,
        },
        VirtualAddress,
    },
};

pub enum ProcessState {
    /// We put a process in the `Poisoned` state while we do work to move it between real states
    /// (e.g. switching to its address space when moving between `NotRunning` and `Running`). This
    /// makes sure we can detect when something went wrong when transistioning between states.
    Poisoned,
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

    pub fn switch_to(&mut self) {
        use core::mem;

        self.state = match mem::replace(&mut self.state, ProcessState::Poisoned) {
            ProcessState::NotRunning(inactive_table) => {
                /*
                 * XXX: `RecursiveMapping` is correct here because we'll always be switching from
                 * either the kernel's or another process' page tables.
                 */
                ProcessState::Running(unsafe { inactive_table.switch_to::<RecursiveMapping>().0 })
            }

            ProcessState::Running(_) => {
                panic!("Tried to switch to a process that is already running!")
            }
            ProcessState::Poisoned => panic!("Tried to switch to a poisoned process!"),
        };
    }
}

/// Drop to Ring 3, into a process. This is used for the initial transition from kernel to user
/// mode after the CPU has been brought up.
pub fn drop_to_usermode(tss: &mut Tss, process: &mut Process) -> ! {
    use x86_64::hw::gdt::{USER_CODE64_SELECTOR, USER_DATA_SELECTOR};

    unsafe {
        /*
         * Disable interrupts so we aren't interrupted in the middle of this. They are
         * re-enabled on the `iret`.
         */
        asm!("cli");

        /*
         * Save the current kernel stack pointer in the TSS.
         */
        let rsp: VirtualAddress;
        asm!(""
         : "={rsp}"(rsp)
         :
         : "rsp"
         : "intel"
        );
        tss.set_kernel_stack(rsp);

        /*
         * Switch to the process' address space.
         */
        process.switch_to();

        /*
         * Jump into Ring 3 by constructing a fake interrupt frame, then returning from the
         * "interrupt".
         */
        asm!("// Push selector for user data segment
              push rax

              // Push new stack pointer
              push rbx

              // Push new RFLAGS. We set this to the bare minimum to avoid leaking flags out of the
              // kernel. Bit 2 must be one, and we enable interrupts by setting bit 9.
              push rcx

              // Push selector for user code segment
              push rdx

              // Push new instruction pointer
              push rsi

              // Zero all the things
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

              // Return from our fake interrupt frame
              iretq
              "
        :
        : "{rax}"(USER_DATA_SELECTOR),
          "{rbx}"(process.threads[0].stack_pointer),
          "{rcx}"(1<<9 | 1<<2),
          "{rdx}"(USER_CODE64_SELECTOR),
          "{rsi}"(process.threads[0].instruction_pointer)
        : "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "r8", "r9", "r10", "r11", "r12", "r13", "r14", "r15"
        : "intel"
        );
        unreachable!();
    }
}

/// This is called when a task yields to the kernel (using `syscall`, on x86_64). Tasks yield when
/// they have no work to do / are waiting on another process to respond to a message etc. The
/// kernel should handle any messages the yielding task has sent, and then schedule another task.
// TODO: think about how we're going to access stuff from here (very annoying). We need to be able
// to dispatch messages etc. (ideally we kinda want to get access to the whole `Arch`).
pub extern "C" fn yield_handler() {
    info!("Task yielded!");
}
