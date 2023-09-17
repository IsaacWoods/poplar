use crate::{scheduler::Scheduler, Platform};
use hal::memory::VAddr;

pub trait PerCpu<P>
where
    P: Platform,
{
    fn scheduler(&mut self) -> &mut Scheduler<P>;
    fn set_kernel_stack_pointer(&mut self, stack_pointer: VAddr);
    fn user_stack_pointer(&self) -> VAddr;
    fn set_user_stack_pointer(&mut self, stack_pointer: VAddr);
}
