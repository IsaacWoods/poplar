use crate::hw::tss::Tss;
use core::{marker::PhantomPinned, pin::Pin};
use hal::{memory::VirtualAddress, PerCpu};
use pin_utils::{unsafe_pinned, unsafe_unpinned};

pub struct PerCpuImpl<T> {
    /// The first field of the per-cpu structure must be a pointer to itself. This is used to access the info by
    /// reading from `gs:0x0`. This means the structure must be pinned, as it is self-referential.
    _self_pointer: *const PerCpuImpl<T>,
    _pin: PhantomPinned,

    /// The next field must then be the current task's kernel stack pointer. We access this manually from assembly
    /// with `gs:0x8`, so it must remain at a fixed offset within this struct.
    current_task_kernel_rsp: VirtualAddress,
    kernel_data: T,

    tss: Tss,
}

impl<T> PerCpuImpl<T> {
    unsafe_unpinned!(current_task_kernel_rsp: VirtualAddress);
    unsafe_pinned!(tss: Tss);
}

impl<T> PerCpu<T> for PerCpuImpl<T> {
    fn kernel_data(mut self: Pin<&mut Self>) -> Pin<&mut T> {
        // NOTE: we have to do this manually (not with pin_utils) for some reason
        unsafe { self.map_unchecked_mut(|per_cpu| &mut per_cpu.kernel_data) }
    }

    fn set_kernel_stack_pointer(mut self: Pin<&mut Self>, stack_pointer: VirtualAddress) {
        *self.as_mut().current_task_kernel_rsp() = stack_pointer;
        self.as_mut().tss().set_kernel_stack(stack_pointer);
    }
}
