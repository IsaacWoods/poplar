// use crate::scheduler::Scheduler;
use core::fmt;

pub struct PerCpu {
    // pub scheduler: Scheduler,
}

impl PerCpu {
    pub fn new() -> PerCpu {
        // PerCpu { scheduler: Scheduler::new() }
        PerCpu {}
    }

    // /// Helper method to get the currently running task. Panics if the kernel hasn't dropped into
    // /// userspace yet.
    // pub fn running_task(&self) -> &WrappedKernelObject<crate::arch_impl::Arch> {
    //     self.scheduler.running_task.as_ref().unwrap()
    // }
}

impl fmt::Debug for PerCpu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // write!(f, "PerCpu(scheduler: {:?})", self.scheduler)
        Ok(())
    }
}
