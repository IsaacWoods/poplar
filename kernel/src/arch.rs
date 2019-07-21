use crate::object::{task::CommonTask, WrappedKernelObject};

/// Trait that is implemented by a type in each architecture module, and passed to `kernel_main`.
/// Provides a common interface to platform-specific operations for the architecture-independent
/// parts of the kernel.
pub trait Architecture: Sized {
    /*
     * Per-arch kernel object representations.
     */
    type AddressSpace;
    type Task: CommonTask;
    type MemoryObject;

    /// Performs the initial kernel -> userspace transistion. Because this doesn't return, it can't
    /// be defined on any of the `KernelObject` types, because then we'd have to hold a lock that
    /// we wouldn't ever be able to release. Instead, we pass this function a Task object, and it
    /// has to carefully manage the locks to make sure they're all released before we jump into
    /// userspace.
    fn drop_to_userspace(&self, task: WrappedKernelObject<Self>) -> !;
}

/// To test some of the kernel's data structures and stuff, we need a type that implements
/// `Architecture`. We define a fake arch, called `FakeArch` to do this.
#[cfg(test)]
pub mod test {
    use crate::{
        arch::Architecture,
        object::{
            task::{CommonTask, TaskState},
            WrappedKernelObject,
        },
    };

    #[derive(PartialEq, Eq, Debug)]
    pub struct FakeArch;

    #[derive(Debug)]
    pub struct FakeTask([u32; 32]);

    impl CommonTask for FakeTask {
        fn state(&self) -> TaskState {
            unimplemented!()
        }

        fn switch_to(&mut self) {
            unimplemented!();
        }
    }

    impl Architecture for FakeArch {
        /*
         * We make some of these large to detect when we're storing the actual data vs. a ref to
         * the object in tests.
         */
        type AddressSpace = [u8; 32];
        type Task = FakeTask;
        type MemoryObject = ();

        fn drop_to_userspace(&self, _: WrappedKernelObject<Self>) -> ! {
            panic!("FakeArch can't drop into userspace")
        }
    }
}
