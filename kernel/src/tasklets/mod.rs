pub mod queue;

use crate::clocksource::Clocksource;
use core::{future::Future, time::Duration};
use maitake::task::JoinHandle;
use spinning_top::Spinlock;

/// Poplar supports running asynchronous tasks (which we call *tasklets* to differentiate from
/// our userspace Task objects) in kernelspace through a [`maitake`](https://github.com/hawkw/mycelium/tree/main/maitake)-based
/// runtime.
pub struct TaskletScheduler {
    scheduler: Spinlock<maitake::scheduler::Scheduler>,
    pub timer: maitake::time::Timer,
}

impl TaskletScheduler {
    pub fn new<T>() -> TaskletScheduler
    where
        T: Clocksource,
    {
        let clock = maitake::time::Clock::new(Duration::from_nanos(1), || T::nanos_since_boot());

        TaskletScheduler {
            scheduler: Spinlock::new(maitake::scheduler::Scheduler::new()),
            timer: maitake::time::Timer::new(clock),
        }
    }

    pub fn spawn<F>(&self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let scheduler = self.scheduler.lock();
        scheduler.spawn(future)
    }

    pub fn tick(&self) {
        self.scheduler.lock().tick();
        self.timer.turn();
    }
}
