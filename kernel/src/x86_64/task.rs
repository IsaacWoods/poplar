use super::{memory::userspace_map, per_cpu::per_cpu_data_mut, Arch};
use crate::object::{
    common::{CommonTask, TaskState},
    WrappedKernelObject,
};
use alloc::{string::String, vec::Vec};
use core::{mem, pin::Pin, str};
use libpebble::caps::Capability;
use x86_64::{boot::ImageInfo, memory::VirtualAddress};

global_asm!(include_str!("task.s"));
extern "C" {
    fn task_entry_trampoline() -> !;

    /// Do the actual context switch: save the context of the old task on its kernel stack, switch
    /// to the new task's kernel stack, restore its context and return. The only non-trivial part
    /// of this is the returning - for tasks that have run before, we simply work our way back up
    /// the kernel callstack and return to userspace from the syscall handler. However, for tasks
    /// that have never been run before, the stack frames leading back up to the handler aren't
    /// there, and so we manually insert a return to a kernel-space usermode trampoline that
    /// enters userspace for the first time in the initial stack frame, which is what the context
    /// switch returns to.
    fn do_context_switch(old_kernel_rsp: *mut VirtualAddress, new_kernel_rsp: VirtualAddress);
}

/// This is the layout of the stack that we expect to be present when we switch to a task. It is
/// created both in preparation for initial task entry, and when we're switching away from a task.
/// We use the C ABI here because we access this structure from assembly.
#[derive(Default, Debug)]
#[repr(C)]
pub struct ContextSwitchFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbp: u64,
    pub rbx: u64,

    /// When we construct an initial stack frame, we set this to the address of the kernel-space
    /// trampoline that enters userspace. On normal context switches, we just push the registers so
    /// this is the real return address that leads back up the kernel call-stack to the syscall
    /// handler.
    pub return_address: u64,
}

#[derive(Debug)]
pub enum TaskCreationError {
    /// The kernel object that should be the AddressSpace that this task will live in is not
    /// actually an AddressSpace
    NotAnAddressSpace,

    /// The AddressSpace has run out of stack slots
    NotEnoughStackSlots,

    /// The task image has an invalid capability encoding
    InvalidCapabilityEncoding,

    /// Initial tasks (from images loaded by the bootloader) can only have names that can be
    /// encoded in UTF-8 in 32 bytes. The name of this task is too long.
    InitialNameTooLong,

    /// The task's name is not valid UTF-8.
    InvalidName,
}

/// This is the representation of a task on x86_64. It's basically just keeps information about the
/// current instruction pointer and the stack, as all other registers are preserved on the task
/// stack when it's suspended.
pub struct Task {
    pub name: String,
    pub address_space: WrappedKernelObject<Arch>,
    pub state: TaskState,
    pub capabilities: Vec<Capability>,

    pub user_stack_top: VirtualAddress,
    pub kernel_stack_top: VirtualAddress,
    pub stack_size: usize,

    /*
     * We only keep track of the kernel stack pointer. The user stack pointer is saved on the
     * kernel stack when we enter the kernel through the `syscall` instruction, and restored before
     * a `sysret`.
     */
    pub kernel_stack_pointer: VirtualAddress,
}

impl Task {
    /// Create a new task in a given address space, which will start executing at the given entry
    /// point when scheduled. This creates a new userspace stack in the given address space.
    ///
    /// ### Panics
    /// * If the given address space doesn't point to a valid `AddressSpace`
    /// * If the `AddressSpace` fails to create a new stack for the task
    pub fn from_image_info(
        arch: &Arch,
        address_space: WrappedKernelObject<Arch>,
        image: &ImageInfo,
    ) -> Result<Task, TaskCreationError> {
        let stack_set = address_space
            .object
            .address_space()
            .ok_or(TaskCreationError::NotAnAddressSpace)?
            .write()
            .add_stack_set(userspace_map::INITIAL_STACK_SIZE, &arch.physical_memory_manager)
            .ok_or(TaskCreationError::NotEnoughStackSlots)?;

        if image.name_length > 32 {
            return Err(TaskCreationError::InitialNameTooLong);
        }
        let name = String::from(
            str::from_utf8(&image.name[0..image.name_length as usize])
                .map_err(|_| TaskCreationError::InvalidName)?,
        );

        let mut task = Task {
            name,
            address_space,
            state: TaskState::Ready,
            capabilities: decode_capabilities(&image.capability_stream)?,
            user_stack_top: stack_set.user_slot_top,
            kernel_stack_top: stack_set.kernel_slot_top,
            stack_size: userspace_map::INITIAL_STACK_SIZE,
            kernel_stack_pointer: stack_set.kernel_slot_top,
        };

        task.initialize_kernel_stack(image.entry_point, stack_set.user_slot_top);
        Ok(task)
    }

    /// Before a task can be started, either by it being scheduled or by dropping into usermode
    /// into it, we need to initialize the kernel stack to make it look like the task was already
    /// running and either yeilded into, or was pre-empted by, the kernel. Then, when we switch to
    /// a task that has never been run before, the stack looks like a normal task, so only one code
    /// path is needed to resume a task.
    fn initialize_kernel_stack(&mut self, task_entry_point: VirtualAddress, user_stack_top: VirtualAddress) {
        // TODO: change this to use the CpuFlags type from our x86_64 crate to create these nicely
        /*
         * These are the set of flags we enter the task for the first time with. We just allow
         * interrupts, and leave everything else at their defaults.
         */
        const INITIAL_RFLAGS: u64 = (1 << 9) | (1 << 2);

        /*
         * Firstly, we need to make sure the top of the stack is 16-byte aligned, according to the
         * Sys-V ABI.
         */
        const REQUIRED_INITIAL_STACK_ALIGNMENT: usize = 16;
        self.kernel_stack_pointer = self.kernel_stack_pointer.align_down(REQUIRED_INITIAL_STACK_ALIGNMENT);

        /*
         * Start off with a zero return address to terminate backtraces at task entry.
         */
        self.kernel_stack_pointer -= 8;
        unsafe {
            *(self.kernel_stack_pointer.mut_ptr() as *mut u64) = 0x0;
        }

        /*
         * Next, we construct the context-switch frame that is used when a task is switched to for
         * the first time. This initializes registers to sensible values, and then jumps to a
         * kernel-space trampoline that enters userspace.
         */
        self.kernel_stack_pointer -= mem::size_of::<ContextSwitchFrame>();
        unsafe {
            *(self.kernel_stack_pointer.mut_ptr() as *mut ContextSwitchFrame) = ContextSwitchFrame {
                r15: usize::from(task_entry_point) as u64,
                r14: INITIAL_RFLAGS,
                r13: usize::from(user_stack_top) as u64,
                r12: 0x0,
                rbp: 0x0,
                rbx: 0x0,
                return_address: task_entry_trampoline as u64,
            };
        }
    }
}

impl CommonTask for Task {
    fn state(&self) -> TaskState {
        self.state
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// Perform a context-switch between the currently running task, `old`, and a new task, `new`. This
/// function is fairly fragile as it may not always return (in the case of the new task never
/// having been run before, in which case this returns into the trampoline instead of back up the
/// kernel callstack), and so we have to be very careful to drop any locks we hold before "returning"
pub fn context_switch(old: WrappedKernelObject<Arch>, new: WrappedKernelObject<Arch>) {
    /*
     * After this scope ends, all locks on the task objects must be dropped, as the call to
     * `do_context_switch` may not return, which means that stuff in the function scope is not
     * dropped.
     */
    let (old_kernel_rsp, new_kernel_rsp): (*mut VirtualAddress, VirtualAddress) = {
        let mut old_task = old.object.task().unwrap().write();
        let mut new_task = new.object.task().unwrap().write();

        assert_eq!(old_task.state, TaskState::Running);
        assert_eq!(new_task.state, TaskState::Ready);
        old_task.state = TaskState::Ready;
        new_task.state = TaskState::Running;

        // Switch to the new task's address space
        old_task.address_space.object.address_space().unwrap().write().switch_away_from();
        new_task.address_space.object.address_space().unwrap().write().switch_to();

        // Install the new kernel stack pointer
        let new_kernel_rsp = new_task.kernel_stack_pointer;
        unsafe {
            Pin::get_unchecked_mut(per_cpu_data_mut().tss_mut()).set_kernel_stack(new_kernel_rsp);
            *per_cpu_data_mut().current_task_kernel_rsp_mut() = new_kernel_rsp;
        }

        ((&mut old_task.kernel_stack_pointer) as *mut VirtualAddress, new_kernel_rsp)
    };

    unsafe {
        do_context_switch(old_kernel_rsp, new_kernel_rsp);
    }
}

/// Drop into usermode into the given task. This permanently migrates from the kernel's initial
/// stack (the one reserved in `.bss` and used during kernel initialization).
// TODO: can this be made into a special case of the scheduling code, where we switch away from
// nothing into a new task, so it just knows not to save the context?
pub fn drop_to_usermode(task: WrappedKernelObject<Arch>) -> ! {
    // We use an inner scope here to make sure we drop the lock on the task object. This function
    // never returns, so it wouldn't be released otherwise.
    let kernel_stack_top = {
        let mut task_object = task.object.task().expect("Not a task").write();
        task_object.state = TaskState::Running;
        task_object.address_space.object.address_space().unwrap().write().switch_to();
        task_object.kernel_stack_pointer
    };

    unsafe {
        /*
         * Set the kernel stack in the TSS to the task's kernel stack. This is safe because
         * changing the kernel stack does not move the TSS.
         */
        Pin::get_unchecked_mut(per_cpu_data_mut().tss_mut()).set_kernel_stack(kernel_stack_top);
        *per_cpu_data_mut().current_task_kernel_rsp_mut() = kernel_stack_top;

        asm!("// First, we disable interrupts so we aren't interrupted. They are enabled when the new flags are
              // loaded by `sysret`
              cli

              // Switch to the task's kernel stack
              mov rsp, gs:0x8

              // Pop the context-saved registers. We zero the rest later (this is cheaper than
              // adding to the stack here and zeroing them all later).
              pop rcx   // The r15 slot contains the instruction pointer, so put it in rcx
              pop r11   // The r14 slot contains the flags, so put it in r11
              pop r13
              pop r12
              pop rbp
              pop rbx

              // The user rsp is put in r13
              mov rsp, r13

              // Zero all registers that aren't already zeroed, except rcx and r11
              // Already zeroed are: rbx, rbp, r12, r13
              xor rax, rax
              mov rdx, rax
              mov rsi, rax
              mov rdi, rax
              mov r8, rax
              mov r9, rax
              mov r10, rax

              // Leap of faith!
              sysretq"
        :
        :
        : "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "rsp", "r8", "r9", "r10", "r11", "r12", "r13", "r14", "r15", "memory"
        : "intel"
        );
        unreachable!();
    }
}

/// Decode a capability stream (as found in a task's image) into a set of capabilities as they're
/// represented in the kernel. For the format that's being decoded here, refer to the
/// `(3.1) Userspace/Capabilities` section of the Book.
// TODO: this shouldn't be here - decoding capabilities is arch-independent
fn decode_capabilities(mut cap_stream: &[u8]) -> Result<Vec<Capability>, TaskCreationError> {
    let mut caps = Vec::new();

    // TODO: when decl_macro hygiene-opt-out is implemented, this should be converted to use it
    macro_rules! one_byte_cap {
        ($cap: path) => {{
            caps.push($cap);
            cap_stream = &cap_stream[1..];
        }};
    }

    while cap_stream.len() > 0 {
        match cap_stream[0] {
            0x01 => one_byte_cap!(Capability::CreateAddressSpace),
            0x02 => one_byte_cap!(Capability::CreateMemoryObject),
            0x03 => one_byte_cap!(Capability::CreateTask),

            0x30 => one_byte_cap!(Capability::AccessBackupFramebuffer),
            0x31 => one_byte_cap!(Capability::EarlyLogging),

            // We skip `0x00` as the first byte of a capability, as it is just used to pad the
            // stream and so has no meaning
            0x00 => cap_stream = &cap_stream[1..],

            _ => return Err(TaskCreationError::InvalidCapabilityEncoding),
        }
    }

    Ok(caps)
}
