#![no_std]
#![feature(const_if_match, decl_macro, step_trait)]

pub mod boot_info;
pub mod memory;

use boot_info::BootInfo;
use core::pin::Pin;
use memory::{FrameSize, PageTable, VirtualAddress};

pub trait Hal<T>: Sized {
    type PageTableSize: FrameSize;
    type PageTable: PageTable<Self::PageTableSize>;
    type TaskHelper: TaskHelper;
    type PerCpu: PerCpu<T>;

    fn init_logger();
    /// Initialise the hardware platform. This is called early on, after initialisation of logging, and the
    /// physical and virtual memory managers. In this function, HAL implementations are expected to initialise all
    /// hardware they can at this stage, and gather any information they need about the platform.
    fn init(boot_info: &BootInfo, per_cpu_data: T) -> Self;

    unsafe fn disable_interrupts();
    unsafe fn enable_interrupts();

    /// Access the per-CPU data as a pinned, mutable reference. This does not take a reference to the HAL, because
    /// it must be callable from contexts that don't have access to the HAL instance. It is unsafe because this
    /// may, depending on HAL implementation behaviour, access uninitialized memory if the per-CPU data hasn't yet
    /// been initialized, and also creates a mutable reference of whatever lifetime is requested. It is the
    /// caller's responsibility to ensure only one mutable reference to the per-CPU data exists at any time (this
    /// is easier than the general case, however, as you only need to think about the current CPU).
    unsafe fn per_cpu<'a>() -> Pin<&'a mut Self::PerCpu>;
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

pub trait PerCpu<T>: Sized {
    fn kernel_data(self: Pin<&mut Self>) -> Pin<&mut T>;
    fn set_kernel_stack_pointer(self: Pin<&mut Self>, stack_pointer: VirtualAddress);
}
