pub mod map;

use crate::arch::Architecture;
#[derive(PartialEq, Eq, Debug)]
pub enum KernelObject<A: Architecture> {
    AddressSpace(A::AddressSpace),

    /// This is a test entry that just allows us to store a number. It is used to test the data
    /// structures that store and interact with kernel objects etc.
    #[cfg(test)]
    Test(usize),
}
