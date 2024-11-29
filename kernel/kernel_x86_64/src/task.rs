use core::{arch::global_asm, mem, ptr};
use hal::memory::VAddr;
use hal_x86_64::hw::registers::{write_msr, CpuFlags};
use kernel::memory::vmm::Stack;

global_asm!(include_str!("task.s"));
global_asm!(include_str!("syscall.s"));
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
    fn do_context_switch(current_kernel_rsp: *mut VAddr, new_kernel_rsp: VAddr);

    /// This function is defined in assembly, and is called when userspace does a `syscall`. It returns back to
    /// userspace using `sysret`, and so is diverging.
    fn syscall_handler() -> !;
}

/// This function is called by `syscall_handler` to enter Rust. This is just required to call the correct `Platform`
/// monomorphization of the common syscall handler.
#[no_mangle]
extern "C" fn rust_syscall_entry(number: usize, a: usize, b: usize, c: usize, d: usize, e: usize) -> usize {
    kernel::syscall::handle_syscall::<crate::PlatformImpl>(
        crate::SCHEDULER.get(),
        crate::KERNEL_PAGE_TABLES.get(),
        number,
        a,
        b,
        c,
        d,
        e,
    )
}

/// This is the layout of the stack that we expect to be present when we switch to a task. It is
/// created both in preparation for initial task entry, and when we're switching away from a task.
/// We use the C ABI here because we access this structure from assembly.
#[derive(Clone, Copy, Default, Debug)]
#[repr(C)]
pub struct ContextSwitchFrame {
    pub flags: u64,
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

/// Returns `(kernel_stack_pointer, user_stack_pointer)`.
pub unsafe fn initialize_stacks(
    kernel_stack: &Stack,
    user_stack: &Stack,
    task_entry_point: VAddr,
) -> (VAddr, VAddr) {
    /*
     * These are the set of flags we enter the task for the first time with. We allow, set the parity flag to
     * even, and leave everything else unset.
     */
    const INITIAL_RFLAGS: CpuFlags =
        CpuFlags::new((1 << CpuFlags::INTERRUPT_ENABLE_FLAG) | (1 << CpuFlags::PARITY_FLAG));

    /*
     * Firstly, we need to make sure the top of the stack is 16-byte aligned, according to the
     * Sys-V ABI.
     */
    const REQUIRED_INITIAL_STACK_ALIGNMENT: usize = 16;
    let mut kernel_stack_pointer = kernel_stack.top.align_down(REQUIRED_INITIAL_STACK_ALIGNMENT);
    let user_stack_pointer = user_stack.top.align_down(REQUIRED_INITIAL_STACK_ALIGNMENT);

    /*
     * Start off with a zero return address to terminate backtraces at task entry.
     */
    kernel_stack_pointer -= 8;
    ptr::write(kernel_stack_pointer.mut_ptr() as *mut u64, 0x0);

    /*
     * Next, we construct the context-switch frame that is used when a task is switched to for
     * the first time. This initializes registers to sensible values, and then jumps to a
     * kernel-space trampoline that enters userspace.
     */
    kernel_stack_pointer -= mem::size_of::<ContextSwitchFrame>();
    ptr::write(
        kernel_stack_pointer.mut_ptr() as *mut ContextSwitchFrame,
        ContextSwitchFrame {
            flags: INITIAL_RFLAGS.into(),
            r15: usize::from(task_entry_point) as u64,
            // TODO: if we keep the new flags thing, revisit this
            r14: INITIAL_RFLAGS.into(),
            r13: 0x0,
            r12: 0x0,
            rbp: 0x0,
            rbx: 0x0,
            return_address: task_entry_trampoline as u64,
        },
    );

    (kernel_stack_pointer, user_stack_pointer)
}

pub unsafe fn context_switch(current_kernel_stack: *mut VAddr, new_kernel_stack: VAddr) {
    do_context_switch(current_kernel_stack, new_kernel_stack);
}

pub unsafe fn drop_into_userspace() -> ! {
    /*
     * On x86_64, we use the context we install into the task's kernel stack to drop into usermode. We don't
     * need the kernel stack pointer as it has already been installed into the per-cpu info, so we can just
     * load it from there.
     */
    do_drop_to_usermode();
}

/// We use the `syscall` instruction to make system calls, as it's always present on supported systems. We need
/// to set a few MSRs to configure how the `syscall` instruction works:
///     - `IA32_LSTAR` contains the address that `syscall` jumps to
///     - `IA32_STAR` contains the segment selectors that `syscall` and `sysret` use. These are not validated
///       against the actual contents of the GDT so these must be correct.
///     - `IA32_FMASK` contains a mask used to set the kernels `rflags`
///
/// To understand the values we set these MSRs to, it helps to look at a (simplified) version of the operation
/// of `syscall`:
/// ```
///     rcx <- rip
///     rip <- IA32_LSTAR
///     r11 <- rflags
///     rflags <- rflags AND NOT(IA32_FMASK)
///
///     cs.selector <- IA32_STAR[47:32] AND 0xfffc          (the AND forces the RPL to 0)
///     cs.base <- 0                                        (note that for speed, the selector is not actually
///     cs.limit <- 0xfffff                                  loaded from the GDT, but hardcoded)
///     cs.type <- 11;
///     (some other fields of the selector, see Intel manual for details)
///
///     cpl <- 0
///
///     ss.selector <- IA32_STAR[47:32] + 8
///     ss.base <- 0
///     ss.limit <- 0xfffff
///     ss.type <- 3
///     (some other fields of the selector)
/// ```
pub fn install_syscall_handler() {
    use bit_field::BitField;
    use hal_x86_64::hw::{
        gdt::{KERNEL_CODE_SELECTOR, USER_COMPAT_CODE_SELECTOR},
        registers::{IA32_FMASK, IA32_LSTAR, IA32_STAR},
    };

    let mut selectors = 0_u64;
    selectors.set_bits(32..48, KERNEL_CODE_SELECTOR.0 as u64);

    /*
     * We put the selector for the Compatibility-mode code segment in here, because `sysret` expects
     * the segments to be in this order:
     *      STAR[48..64]        => 32-bit Code Segment
     *      STAR[48..64] + 8    => Data Segment
     *      STAR[48..64] + 16   => 64-bit Code Segment
     */
    selectors.set_bits(48..64, USER_COMPAT_CODE_SELECTOR.0 as u64);

    /*
     * Upon `syscall`, `rflags` is moved into `r11`, and then the bits set in this mask are cleared. We
     * clear some stuff so the kernel runs in a sensible environment, regardless of what usermode is
     * doing.
     *
     * Importantly, we disable interrupts because they're not safe until we've stopped messing about with
     * stacks.
     */
    let flags_mask = CpuFlags::STATUS_MASK
        | CpuFlags::TRAP_FLAG
        | CpuFlags::INTERRUPT_ENABLE_FLAG
        | CpuFlags::IO_PRIVILEGE_MASK
        | CpuFlags::NESTED_TASK_FLAG
        | CpuFlags::ALIGNMENT_CHECK_FLAG;

    unsafe {
        write_msr(IA32_STAR, selectors);
        write_msr(IA32_LSTAR, syscall_handler as u64);
        write_msr(IA32_FMASK, flags_mask);
    }
}
