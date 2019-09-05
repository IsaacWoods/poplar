pub mod common;
pub mod map;

use self::map::ObjectMap;
use crate::arch::Architecture;
use alloc::{boxed::Box, sync::Arc};
use core::fmt;
use libpebble::KernelObjectId;
use spin::RwLock;

pub struct WrappedKernelObject<A: Architecture> {
    pub id: KernelObjectId,
    pub object: Arc<KernelObject<A>>,
}

impl<A> fmt::Debug for WrappedKernelObject<A>
where
    A: Architecture,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KernelObject(id = {:?})", self.id)
    }
}

impl<A> Clone for WrappedKernelObject<A>
where
    A: Architecture,
{
    fn clone(&self) -> Self {
        WrappedKernelObject { id: self.id, object: self.object.clone() }
    }
}

// TODO: when unhygenic macro items are implemented, we should just be able to do `enum
// #KernelObject`, `#Test`, and `#wrap` and not have to pass parameters like this
macro kernel_object_table($kernel_object: ident, $test: ident, $add_to_map: ident, $([$name: ident, $method: ident]),*) {
    #[derive(Debug)]
    pub enum $kernel_object<A: Architecture> {
        $(
            $name(RwLock<Box<A::$name>>),
         )*

        /// This is a test entry that just allows us to store a number. It is used to test the data
        /// structures that store and interact with kernel objects etc.
        #[cfg(test)]
        $test(usize),
    }

    impl<A> $kernel_object<A> where A: Architecture {
        pub fn $add_to_map(self, map: &mut ObjectMap<A>) -> WrappedKernelObject<A> {
            let wrapped_object = Arc::new(self);
            let id = map.insert(wrapped_object.clone());
            WrappedKernelObject { id, object: wrapped_object, }
        }

        $(
            pub fn $method(&self) -> Option<&RwLock<Box<A::$name>>> {
                match self {
                    KernelObject::$name(ref object) => Some(object),
                    _ => None,
                }
            }
         )*
    }
}

kernel_object_table!(
    KernelObject,
    Test,
    add_to_map,
    [AddressSpace, address_space],
    [MemoryObject, memory_object],
    [Task, task]
);

/// Make sure that `KernelObject` doesn't get bigger without us thinking about it
#[test]
fn kernel_object_not_too_big() {
    assert_eq!(core::mem::size_of::<KernelObject<crate::arch::test::FakeArch>>(), 24);
}
