use crate::{scheduler::Scheduler, Platform};
use core::pin::Pin;
use hal::memory::VirtualAddress;

pub trait PerCpu<P>
where
    P: Platform,
{
    fn scheduler(self: Pin<&mut Self>) -> Pin<&mut Scheduler<P>>;
    fn set_kernel_stack_pointer(self: Pin<&mut Self>, stack_pointer: VirtualAddress);
    fn set_user_stack_pointer(self: Pin<&mut Self>, stack_pointer: VirtualAddress);
}
