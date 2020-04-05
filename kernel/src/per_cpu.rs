use crate::{scheduler::Scheduler, HalImpl};
use pebble_util::unsafe_unpinned;

pub struct KernelPerCpu {
    pub scheduler: Scheduler<HalImpl>,
}

impl KernelPerCpu {
    unsafe_unpinned!(pub scheduler: Scheduler<HalImpl>);
}
