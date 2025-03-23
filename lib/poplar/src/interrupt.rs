use crate::{
    syscall::{self, WaitForInterruptError},
    Handle,
};
use core::{future::Future, task::Poll};

pub struct Interrupt(Handle);

impl Interrupt {
    pub fn new_from_handle(handle: Handle) -> Interrupt {
        Interrupt(handle)
    }

    pub fn wait_for_interrupt(&self) -> impl Future<Output = ()> + '_ {
        core::future::poll_fn(|context| {
            /*
             * We call `wait_for_interrupt`, but don't allow it to block. This effectively just clears
             * the interrupt if there is one pending to be handled - the async side handles waiting for
             * events through `poll_interest` via the reactor.
             */
            match syscall::wait_for_interrupt(self.0, false) {
                Ok(()) => Poll::Ready(()),
                Err(WaitForInterruptError::NoInterrupt) => {
                    crate::rt::RUNTIME.get().reactor.lock().register(self.0, context.waker().clone());
                    Poll::Pending
                }
                Err(other) => panic!("Error waiting for interrupt: {:?}", other),
            }
        })
    }

    pub fn wait_for_interrupt_blocking(&self) {
        syscall::wait_for_interrupt(self.0, true).unwrap();
    }

    pub fn ack(&self) {
        if let Err(err) = syscall::ack_interrupt(self.0) {
            panic!("Issue acknowledging interrupt: {:?}", err);
        }
    }
}
