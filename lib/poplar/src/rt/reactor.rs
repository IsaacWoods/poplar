use crate::Handle;
use alloc::{collections::BTreeMap, vec::Vec};
use core::task::Waker;

/// The `Reactor` is a component of the Poplar userspace async runtime that processes events from
/// kernel objects in order to wake futures when they have work to do.
pub struct Reactor {
    interests: BTreeMap<Handle, Waker>,
}

impl Reactor {
    pub fn new() -> Reactor {
        Reactor { interests: BTreeMap::new() }
    }

    pub fn register(&mut self, handle: Handle, waker: Waker) {
        self.interests.insert(handle, waker);
    }

    pub fn poll(&mut self) {
        /*
         * Make a copy of the current list of handles we're interested in. We do this so we can
         * later remove events that have been awoken.
         */
        let handles: Vec<Handle> = self.interests.keys().copied().collect();

        for handle in handles {
            if crate::syscall::poll_interest(handle).unwrap() {
                let waker = self.interests.remove(&handle).unwrap();
                waker.wake();
            }
        }
    }
}
