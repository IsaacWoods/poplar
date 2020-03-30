use crate::hw::{
    gdt::{SegmentSelector, TssSegment},
    tss::Tss,
};
use alloc::boxed::Box;
use core::{marker::PhantomPinned, mem, pin::Pin};
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

    pub fn new(kernel_data: T) -> (Pin<Box<PerCpuImpl<T>>>, SegmentSelector) {
        use crate::hw::registers::{write_msr, IA32_GS_BASE};

        let tss = Tss::new();
        let mut per_cpu = Box::pin(PerCpuImpl {
            _self_pointer: 0x0 as *const PerCpuImpl<T>,
            _pin: PhantomPinned,

            current_task_kernel_rsp: VirtualAddress::new(0x0),
            kernel_data,

            tss,
        });

        /*
         * Install the TSS into the GDT.
         */
        let tss_selector = crate::hw::gdt::GDT.lock().add_tss(TssSegment::new(per_cpu.as_mut().tss().into_ref()));

        /*
         * Fill out the self-pointer, and then install it into the MSR so we can access it using `gs`.
         */
        let address: *const PerCpuImpl<T> = unsafe { mem::transmute(per_cpu.as_ref()) };
        unsafe {
            Pin::get_unchecked_mut(Pin::as_mut(&mut per_cpu))._self_pointer = address;
            write_msr(IA32_GS_BASE, address as usize as u64);
        }

        (per_cpu, tss_selector)
    }
}

impl<T> PerCpu<T> for PerCpuImpl<T> {
    fn kernel_data(mut self: Pin<&mut Self>) -> Pin<&mut T> {
        // XXX: we have to do this manually (not with pin_utils) for some reason
        unsafe { self.map_unchecked_mut(|per_cpu| &mut per_cpu.kernel_data) }
    }

    fn set_kernel_stack_pointer(mut self: Pin<&mut Self>, stack_pointer: VirtualAddress) {
        *self.as_mut().current_task_kernel_rsp() = stack_pointer;
        self.as_mut().tss().set_kernel_stack(stack_pointer);
    }
}
