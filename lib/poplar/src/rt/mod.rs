//! Poplar's `async` runtime. This provides an executor based on
//! [`maitake`](https://github.com/hawkw/mycelium/tree/main/maitake) and a reactor compatible with
//! Poplar's system call layer.

mod reactor;

use self::reactor::Reactor;
use core::future::Future;
use maitake::{scheduler::Scheduler, task::JoinHandle};
use poplar_util::InitGuard;
use spinning_top::Spinlock;

// TODO: if we want support for multiple tasks in an address space, this needs to be thread-local
pub(crate) static RUNTIME: InitGuard<Runtime> = InitGuard::uninit();

pub struct Runtime {
    scheduler: Scheduler,
    // TODO: maintain a timer wheel so time-based futures work in userspace
    pub reactor: Spinlock<Reactor>,
}

pub fn init_runtime() {
    RUNTIME.initialize(Runtime { scheduler: Scheduler::new(), reactor: Spinlock::new(Reactor::new()) });
}

pub fn enter_loop() {
    loop {
        crate::syscall::yield_to_kernel();

        let runtime = RUNTIME.get();
        runtime.reactor.lock().poll();
        runtime.scheduler.tick();
    }
}

pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    RUNTIME.get().scheduler.spawn(future)
}
