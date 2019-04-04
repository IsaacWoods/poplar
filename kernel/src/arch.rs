/// Trait that is implemented by a type in each architecture module, and passed to `kernel_main`.
/// Provides a common interface to platform-specific operations for the architecture-independent
/// parts of the kernel.
pub trait Architecture {
    type AddressSpace;
    type Task;
}

/// To test some of the kernel's data structures and stuff, we need a type that implements
/// `Architecture`. We define a fake arch, called `FakeArch` to do this.
#[cfg(test)]
pub mod test {
    use super::Architecture;

    #[derive(PartialEq, Eq, Debug)]
    pub struct FakeArch;

    impl Architecture for FakeArch {
        type AddressSpace = ();
        type Task = ();
    }
}
