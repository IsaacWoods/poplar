use alloc::boxed::Box;
use core::{arch::asm, ptr};
use hal::memory::VAddr;
use hal_x86_64::hw::tss::Tss;
use kernel::{per_cpu::PerCpu, scheduler::Scheduler};

/// Get a mutable reference to the per-CPU data of the running CPU. This is unsafe because it is the caller's
/// responsibility to ensure that only one mutable reference to the per-CPU data exists at any one time. It is also
/// unsafe to call this before the per-CPU data has been installed.
pub unsafe fn get_per_cpu_data<'a>() -> &'a mut PerCpuImpl {
    let mut ptr: usize;
    asm!("mov {}, gs:0x0", out(reg) ptr);
    tracing::info!("Reading per-cpu data from {:#x}", ptr);
    &mut *(ptr as *mut PerCpuImpl)
}

/// Represents data that is held individually for each CPU.
///
/// Per-CPU data on x86_64 is accessed by reading a pointer to itself from the start of the structure. Various
/// fields of this structure are accessed directly from assembly, and so it is essential that field padding and
/// reordering are avoided (and so we use `repr(C)`).
// TODO: this structure is self-referential, and so should be really be pinned, but this was a pain so we avoided
// it. Maybe review this at some point / if it ends up causing UB.
#[repr(C)]
pub struct PerCpuImpl {
    /// The first field of the per-cpu structure must be a pointer to itself. This is used to access the info by
    /// reading from `gs:0x0`. This means the structure is self-referential.
    _self_pointer: *mut PerCpuImpl,

    /// The next field must then be the current task's kernel stack pointer. We access this manually from assembly
    /// with `gs:0x8`, so it must remain at a fixed offset within this struct.
    current_task_kernel_rsp: VAddr,
    /// This field must remain at `gs:0x10`, and so cannot be moved.
    current_task_user_rsp: VAddr,

    pub tss: Box<Tss>,

    scheduler: Scheduler<crate::PlatformImpl>,
}

impl PerCpuImpl {
    pub fn install(tss: Box<Tss>, scheduler: Scheduler<crate::PlatformImpl>) {
        use hal_x86_64::hw::registers::{write_msr, IA32_GS_BASE};

        let per_cpu = Box::new(PerCpuImpl {
            _self_pointer: 0x0 as *mut PerCpuImpl,

            current_task_kernel_rsp: VAddr::new(0x0),
            current_task_user_rsp: VAddr::new(0x0),
            tss,

            scheduler,
        });
        let address = Box::into_raw(per_cpu) as usize;

        // Now we know the address of the structure, fill in the self-pointer.
        unsafe {
            ptr::write(address as *mut usize, address as usize);
        }

        unsafe {
            write_msr(IA32_GS_BASE, address as u64);
        }
    }
}

impl PerCpu<crate::PlatformImpl> for PerCpuImpl {
    fn scheduler(&mut self) -> &mut Scheduler<crate::PlatformImpl> {
        &mut self.scheduler
    }

    fn set_kernel_stack_pointer(&mut self, stack_pointer: VAddr) {
        self.current_task_kernel_rsp = stack_pointer;
        self.tss.set_kernel_stack(stack_pointer);
    }

    fn user_stack_pointer(&self) -> VAddr {
        self.current_task_user_rsp
    }

    fn set_user_stack_pointer(&mut self, stack_pointer: VAddr) {
        self.current_task_user_rsp = stack_pointer;
    }
}

impl Drop for PerCpuImpl {
    fn drop(&mut self) {
        panic!("Per-CPU data should not be dropped!");
    }
}
