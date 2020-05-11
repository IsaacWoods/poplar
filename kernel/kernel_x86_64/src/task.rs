use core::mem;
use hal::memory::VirtualAddress;

global_asm!(include_str!("task.s"));
extern "C" {
    fn task_entry_trampoline() -> !;

    fn do_drop_to_usermode() -> !;

    /// Do the actual context switch: save the context of the old task on its kernel stack, switch
    /// to the new task's kernel stack, restore its context and return. The only non-trivial part
    /// of this is the returning - for tasks that have run before, we simply work our way back up
    /// the kernel callstack and return to userspace from the syscall handler. However, for tasks
    /// that have never been run before, the stack frames leading back up to the handler aren't
    /// there, and so we manually insert a return to a kernel-space usermode trampoline that
    /// enters userspace for the first time in the initial stack frame, which is what the context
    /// switch returns to.
    fn do_context_switch(current_kernel_rsp: *mut VirtualAddress, new_kernel_rsp: VirtualAddress);
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

pub unsafe fn initialize_kernel_stack(
    kernel_stack_top: &mut VirtualAddress,
    task_entry_point: VirtualAddress,
    mut user_stack_top: VirtualAddress,
) {
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
    *kernel_stack_top = kernel_stack_top.align_down(REQUIRED_INITIAL_STACK_ALIGNMENT);
    user_stack_top = user_stack_top.align_down(REQUIRED_INITIAL_STACK_ALIGNMENT);

    /*
     * Start off with a zero return address to terminate backtraces at task entry.
     */
    *kernel_stack_top -= 8;
    *(kernel_stack_top.mut_ptr() as *mut u64) = 0x0;

    /*
     * Next, we construct the context-switch frame that is used when a task is switched to for
     * the first time. This initializes registers to sensible values, and then jumps to a
     * kernel-space trampoline that enters userspace.
     */
    *kernel_stack_top -= mem::size_of::<ContextSwitchFrame>();
    *(kernel_stack_top.mut_ptr() as *mut ContextSwitchFrame) = ContextSwitchFrame {
        r15: usize::from(task_entry_point) as u64,
        r14: INITIAL_RFLAGS,
        r13: usize::from(user_stack_top) as u64,
        r12: 0x0,
        rbp: 0x0,
        rbx: 0x0,
        return_address: task_entry_trampoline as u64,
    };
}

pub unsafe fn context_switch(current_kernel_stack: *mut VirtualAddress, new_kernel_stack: VirtualAddress) {
    do_context_switch(current_kernel_stack, new_kernel_stack);
}

pub unsafe fn drop_into_userspace(_kernel_stack_pointer: VirtualAddress) -> ! {
    /*
     * On x86_64, we use the context we install into the task's kernel stack to drop into usermode. We don't
     * need the kernel stack pointer as it has already been installed into the per-cpu info, so we can just
     * load it from there.
     */
    do_drop_to_usermode();
}
