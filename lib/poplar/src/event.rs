use crate::{
    syscall::{self, WaitForEventError},
    Handle,
};
use core::{future::Future, task::Poll};

pub struct Event(Handle);

impl Event {
    pub fn new_from_handle(handle: Handle) -> Event {
        Event(handle)
    }

    pub fn wait_for_event(&self) -> impl Future<Output = ()> + '_ {
        core::future::poll_fn(|context| {
            /*
             * We call `wait_for_event`, but don't allow it to block. This effectively just clears
             * the event if there is one pending to be handled - the async side handles waiting for
             * events through `poll_interest` via the reactor.
             */
            match syscall::wait_for_event(self.0, false) {
                Ok(()) => Poll::Ready(()),
                Err(WaitForEventError::NoEvent) => {
                    crate::rt::RUNTIME.get().reactor.lock().register(self.0, context.waker().clone());
                    Poll::Pending
                }
                Err(other) => panic!("Error waiting for event: {:?}", other),
            }
        })
    }

    pub fn wait_for_event_blocking(&self) {
        syscall::wait_for_event(self.0, true).unwrap();
    }
}
