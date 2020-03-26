#![no_std]
#![feature(const_if_match, decl_macro, step_trait)]

pub mod boot_info;
pub mod memory;

use boot_info::BootInfo;
use memory::{FrameAllocator, FrameSize, PageTable, VirtualAddress};

pub trait Hal: Sized {
    type PageTableSize: FrameSize;
    type TableAllocator: FrameAllocator<Self::PageTableSize>;
    type PageTable: PageTable<Self::PageTableSize, Self::TableAllocator>;
    type TaskHelper: TaskHelper;

    fn init_logger();
    fn new(boot_info: &BootInfo) -> Self;

    unsafe fn disable_interrupts();
    unsafe fn enable_interrupts();
}

pub trait TaskHelper {
    /// Often, the kernel stack of a task must be initialized to allow it to enter usermode for the first time.
    /// What is required for this is architecture-dependent, and so this is offloaded to the `TaskHelper`.
    ///
    /// `entry_point` is the address that should be jumped to in usermode when the task is run for the first time.
    /// `user_stack_top` is the virtual address that should be put into the stack pointer when the task is entered.
    ///
    /// `kernel_stack_top` is the kernel stack that the new stack frames will be installed in, and must be mapped
    /// and writable when this is called. This method will update it as it puts stuff on the kernel stack.
    fn initialize_kernel_stack(
        kernel_stack_top: &mut VirtualAddress,
        task_entry_point: VirtualAddress,
        user_stack_top: VirtualAddress,
    );

    /// Do the final part of a context switch: save all the state that needs to be to the current kernel stack,
    /// switch to a new kernel stack, and restore all the state from that stack.
    fn context_switch(current_kernel_stack: &mut VirtualAddress, new_kernel_stack: VirtualAddress);
}
