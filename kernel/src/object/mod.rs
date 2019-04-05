pub mod map;

use crate::arch::Architecture;
use spin::RwLock;

// TODO: when unhygenic macro items are implemented, we should just be able to do `enum
// #KernelObject` and `#Test` and not have to pass parameters like this
macro kernel_object_table($kernel_object: ident, $test: ident, $([$name: ident, $method: ident]),*) {
    #[derive(Debug)]
    pub enum $kernel_object<A: Architecture> {
        $(
            $name(RwLock<A::$name>),
         )*

        /// This is a test entry that just allows us to store a number. It is used to test the data
        /// structures that store and interact with kernel objects etc.
        #[cfg(test)]
        $test(usize),
    }

    impl<A> KernelObject<A> where A: Architecture {
        $(
            // TODO: should this actually just return an Option<...> instead?
            pub fn $method(&self) -> &RwLock<A::$name> {
                match self {
                    KernelObject::$name(ref object) => object,
                    _ => panic!("Tried to coerce kernel object into incorrect type!"),
                }
            }
         )*
    }
}

kernel_object_table!(KernelObject, Test, [AddressSpace, address_space], [Task, task]);

/// Make sure that `KernelObject` doesn't get bigger without us thinking about it
#[test]
fn kernel_object_not_too_big() {
    assert_eq!(core::mem::size_of::<KernelObject<crate::arch::test::FakeArch>>(), 16);
}
