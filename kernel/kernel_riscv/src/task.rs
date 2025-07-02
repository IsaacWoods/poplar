use core::{
    arch::{asm, global_asm},
    cell::{Cell, SyncUnsafeCell},
    ptr,
};
use hal::memory::VAddr;
use hal_riscv::hw::csr::Sscratch;
use kernel::vmm::Stack;

global_asm!(include_str!("task.s"));
extern "C" {
    fn task_entry_trampoline() -> !;
    fn do_drop_to_userspace(context: *const ContextSwitchFrame) -> !;
    fn do_context_switch(from_context: *mut ContextSwitchFrame, to_context: *const ContextSwitchFrame);
}

static SCRATCH: SyncUnsafeCell<Scratch> = SyncUnsafeCell::new(Scratch {
    kernel_stack_pointer: VAddr::new(0x0),
    kernel_thread_pointer: VAddr::new(0x0),
    kernel_global_pointer: VAddr::new(0x0),
    scratch_stack_pointer: VAddr::new(0x0),
});

/*
 * XXX: the offsets of fields in this struct are used in assembly, so care must be taken when
 * re-ordering / adding fields.
 */
pub struct Scratch {
    pub kernel_stack_pointer: VAddr,
    pub kernel_thread_pointer: VAddr,
    pub kernel_global_pointer: VAddr,
    pub scratch_stack_pointer: VAddr,
}

impl Scratch {
    pub fn new(kernel_stack_pointer: VAddr) -> Scratch {
        Scratch {
            kernel_stack_pointer,
            kernel_thread_pointer: tp(),
            kernel_global_pointer: gp(),
            scratch_stack_pointer: VAddr::new(0x0),
        }
    }
}

pub fn tp() -> VAddr {
    let value: usize;
    unsafe {
        asm!("mv {}, tp", out(reg) value);
    }
    VAddr::new(value)
}

pub fn gp() -> VAddr {
    let value: usize;
    unsafe {
        asm!("mv {}, gp", out(reg) value);
    }
    VAddr::new(value)
}

#[derive(Clone, Copy, Default, Debug)]
#[repr(C)]
pub struct ContextSwitchFrame {
    pub ra: usize,
    pub sp: usize,
    pub s0: usize,
    pub s1: usize,
    pub s2: usize,
    pub s3: usize,
    pub s4: usize,
    pub s5: usize,
    pub s6: usize,
    pub s7: usize,
    pub s8: usize,
    pub s9: usize,
    pub s10: usize,
    pub s11: usize,
}

/// The context stored for each task. On RISC-V, we store the context switch state in the task
/// context.
pub struct TaskContext {
    context_switch_frame: ContextSwitchFrame,
    kernel_stack_pointer: VAddr,
}

pub fn new_task_context(kernel_stack: &Stack, user_stack: &Stack, task_entry_point: VAddr) -> TaskContext {
    /*
     * Initialize the kernel stack. Firstly, we need to make sure the top of the stack is 16-byte
     * aligned, according to the Sys-V ABI.
     */
    const REQUIRED_INITIAL_STACK_ALIGNMENT: usize = 16;
    let mut kernel_stack_pointer = kernel_stack.top.align_down(REQUIRED_INITIAL_STACK_ALIGNMENT);
    let user_stack_pointer = user_stack.top.align_down(REQUIRED_INITIAL_STACK_ALIGNMENT);

    /*
     * Start off with a zero return address to terminate backtraces at task entry.
     */
    kernel_stack_pointer -= 8;
    unsafe { ptr::write(kernel_stack_pointer.mut_ptr() as *mut u64, 0x0) };

    let context_switch_frame = ContextSwitchFrame {
        ra: task_entry_trampoline as usize,
        sp: usize::from(kernel_stack_pointer),
        s0: usize::from(task_entry_point),
        s1: usize::from(user_stack_pointer),
        s2: 0,
        s3: 0,
        s4: 0,
        s5: 0,
        s6: 0,
        s7: 0,
        s8: 0,
        s9: 0,
        s10: 0,
        s11: 0,
    };

    TaskContext { context_switch_frame, kernel_stack_pointer }
}

pub unsafe fn context_switch(from_context: *mut TaskContext, to_context: *const TaskContext) {
    unsafe {
        (*from_context).kernel_stack_pointer = (*SCRATCH.get()).kernel_stack_pointer;
        let new_kernel_stack_pointer = (*to_context).kernel_stack_pointer;
        *SCRATCH.get() = Scratch::new(new_kernel_stack_pointer);
    }
    do_context_switch(
        &raw mut (*from_context).context_switch_frame,
        &raw const (*to_context).context_switch_frame,
    );
}

pub unsafe fn drop_into_userspace(context: *const TaskContext) -> ! {
    // Initialize this HART's `sscratch` area
    unsafe {
        let kernel_stack_pointer = (*context).kernel_stack_pointer;
        *SCRATCH.get() = Scratch::new(kernel_stack_pointer);
    }
    Sscratch::write(VAddr::from(SCRATCH.get()));

    unsafe { do_drop_to_userspace(&raw const (*context).context_switch_frame) }
}
