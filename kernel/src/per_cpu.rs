// use crate::scheduler::Scheduler;
use core::fmt;

pub struct KernelPerCpu {
    // pub scheduler: Scheduler,
}

impl KernelPerCpu {
    pub fn new() -> KernelPerCpu {
        // PerCpu { scheduler: Scheduler::new() }
        KernelPerCpu {}
    }

    // /// Helper method to get the currently running task. Panics if the kernel hasn't dropped into
    // /// userspace yet.
    // pub fn running_task(&self) -> &WrappedKernelObject<crate::arch_impl::Arch> {
    //     self.scheduler.running_task.as_ref().unwrap()
    // }
}

// TODO: why doesn't this just derive Debug?
// impl fmt::Debug for PerCpu {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         // write!(f, "PerCpu(scheduler: {:?})", self.scheduler)
//         Ok(())
//     }
// }
