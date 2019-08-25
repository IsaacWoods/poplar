use crate::{object::WrappedKernelObject, scheduler::Scheduler};
use core::fmt;

/// Per-cpu data that needs to be accessed from the arch-independent parts of the kernel. This
/// structure should be contained within an arch-specific structure defined in each arch module
/// that is installed as the actual per-cpu data structure. It should then be exposed by two
/// functions, `common_per_cpu` and `common_per_cpu_mut`, from each arch module for the rest of the
/// kernel to use.
pub struct CommonPerCpu {
    pub scheduler: Scheduler,
}

impl CommonPerCpu {
    pub fn new() -> CommonPerCpu {
        CommonPerCpu { scheduler: Scheduler::new() }
    }

    /// Helper method to get the currently running task. Panics if the kernel hasn't dropped into
    /// userspace yet.
    pub fn running_task(&self) -> &WrappedKernelObject<crate::arch_impl::Arch> {
        self.scheduler.running_task.as_ref().unwrap()
    }
}

impl fmt::Debug for CommonPerCpu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CommonPerCpu(scheduler: {:?})", self.scheduler)
    }
}
