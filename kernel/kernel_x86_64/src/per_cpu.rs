use alloc::boxed::Box;
use core::{marker::PhantomPinned, mem, pin::Pin};
use hal::memory::VirtualAddress;
use hal_x86_64::hw::{
    gdt::{SegmentSelector, TssSegment},
    tss::Tss,
};
use kernel::{per_cpu::PerCpu, scheduler::Scheduler};
use pebble_util::{unsafe_pinned, unsafe_unpinned};

/// Get a mutable reference to the per-CPU data of the running CPU. This is unsafe because it is the caller's
/// responsibility to ensure that only one mutable reference to the per-CPU data exists at any one time. It is also
/// unsafe to call this before the per-CPU data has been installed.
pub unsafe fn get_per_cpu_data<'a>() -> Pin<&'a mut PerCpuImpl> {
    let mut ptr: usize;
    llvm_asm!("mov $0, gs:0x0"
        : "=r"(ptr)
        :
        :
        : "intel", "volatile"
    );
    Pin::new_unchecked(&mut *(ptr as *mut PerCpuImpl))
}

pub struct PerCpuImpl {
    /// The first field of the per-cpu structure must be a pointer to itself. This is used to access the info by
    /// reading from `gs:0x0`. This means the structure must be pinned, as it is self-referential.
    _self_pointer: *const PerCpuImpl,
    _pin: PhantomPinned,

    /// The next field must then be the current task's kernel stack pointer. We access this manually from assembly
    /// with `gs:0x8`, so it must remain at a fixed offset within this struct.
    current_task_kernel_rsp: VirtualAddress,
    /// This field must remain at `gs:0x10`, and so cannot be moved.
    current_task_user_rsp: VirtualAddress,

    tss: Tss,

    scheduler: Scheduler<crate::PlatformImpl>,
}

impl PerCpuImpl {
    unsafe_unpinned!(current_task_kernel_rsp: VirtualAddress);
    unsafe_unpinned!(current_task_user_rsp: VirtualAddress);
    unsafe_pinned!(tss: Tss);
    unsafe_pinned!(pub scheduler: Scheduler<crate::PlatformImpl>);

    pub fn new(scheduler: Scheduler<crate::PlatformImpl>) -> (Pin<Box<PerCpuImpl>>, SegmentSelector) {
        let tss = Tss::new();
        let mut per_cpu = Box::pin(PerCpuImpl {
            _self_pointer: 0x0 as *const PerCpuImpl,
            _pin: PhantomPinned,

            current_task_kernel_rsp: VirtualAddress::new(0x0),
            current_task_user_rsp: VirtualAddress::new(0x0),
            tss,

            scheduler,
        });

        /*
         * Install the TSS into the GDT.
         */
        let tss_selector =
            hal_x86_64::hw::gdt::GDT.lock().add_tss(TssSegment::new(per_cpu.as_mut().tss().into_ref()));

        /*
         * Fill out the self-pointer, and then install it into the MSR so we can access it using `gs`.
         */
        let address: *mut PerCpuImpl = unsafe { mem::transmute(per_cpu.as_ref()) };
        unsafe {
            Pin::get_unchecked_mut(per_cpu.as_mut())._self_pointer = address;
        }

        (per_cpu, tss_selector)
    }

    pub fn install(self: Pin<&mut Self>) {
        use hal_x86_64::hw::registers::{write_msr, IA32_GS_BASE};

        unsafe {
            write_msr(IA32_GS_BASE, self.as_ref()._self_pointer as usize as u64);
        }
    }
}

impl PerCpu<crate::PlatformImpl> for PerCpuImpl {
    fn scheduler(self: Pin<&mut Self>) -> Pin<&mut Scheduler<crate::PlatformImpl>> {
        self.scheduler()
    }

    fn set_kernel_stack_pointer(mut self: Pin<&mut Self>, stack_pointer: VirtualAddress) {
        *self.as_mut().current_task_kernel_rsp() = stack_pointer;
        self.as_mut().tss().set_kernel_stack(stack_pointer);
    }

    fn set_user_stack_pointer(mut self: Pin<&mut Self>, stack_pointer: VirtualAddress) {
        *self.as_mut().current_task_user_rsp() = stack_pointer;
    }
}

/*
 * Accidently dropping the per-CPU data after it's been installed leads to some really weird behaviour that I've
 * found difficult to debug in the past, so this guards against that.
 */
impl Drop for PerCpuImpl {
    fn drop(&mut self) {
        panic!("Per-CPU data should not be dropped!");
    }
}
