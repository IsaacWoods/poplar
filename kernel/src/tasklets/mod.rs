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
    pub fn new() -> TaskletScheduler {
        TaskletScheduler {
            scheduler: Spinlock::new(maitake::scheduler::Scheduler::new()),
            // TODO: probs need to be able to supply a tick granularity?
            timer: maitake::time::Timer::new(Duration::from_millis(20)),
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
    }

    pub fn advance_timer(&self, ticks: u64) {
        self.timer.advance_ticks(1);
    }
}
