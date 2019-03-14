pub mod map;

use crate::arch::Architecture;
use alloc::sync::Arc;
use spin::Mutex;
#[derive(Debug)]
pub enum KernelObject<A: Architecture> {
    AddressSpace(A::AddressSpace),

    /// This is a test entry that just allows us to store a number. It is used to test the data
    /// structures that store and interact with kernel objects etc.
    #[cfg(test)]
    Test(usize),
}

/// Make sure that `KernelObject` doesn't get bigger without us thinking about it
#[test]
fn kernel_object_not_too_big() {
    assert_eq!(core::mem::size_of::<KernelObject<crate::arch::test::FakeArch>>(), 16);
}
