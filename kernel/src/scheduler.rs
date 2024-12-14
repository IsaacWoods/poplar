use crate::{
    object::task::{Task, TaskState},
    tasklets::TaskletScheduler,
    Platform,
};
use alloc::{collections::VecDeque, sync::Arc, vec::Vec};
use spinning_top::{guard::SpinlockGuard, Spinlock};
use tracing::{info, trace};

/// The global `Scheduler` coordinates the main 'run loop' of the kernel, allocating CPU time to
/// userspace tasks. There is one global `Scheduler` instance, which then holds a `CpuScheduler`
/// for each running processor to coordinate tasks running on that processor.
///
/// It is also responsible for managing spawned kernel asynchronous tasklets (which are somewhat
/// confusingly also often called `Task`s) - this involves tracking tasks that have been 'woken'
/// (are ready to make progress) and making sure they are polled regularly. The forward progress of
/// both userspace tasks and kernel tasklets are intertwined, and so are managed together.
pub struct Scheduler<P>
where
    P: Platform,
{
    // TODO: in the future, this will be a vec with a CpuScheduler for each CPU
    task_scheduler: Spinlock<CpuScheduler<P>>,
    // TODO: have a maitake scheduler for each processor (ACTUALLY I can't work out if we need one
    // - LocalScheduler could be the core-local one, but both say single-core... Maybe we can just
    // have one and tick it from whatever processor is available?)
    pub tasklet_scheduler: TaskletScheduler,
}

pub struct CpuScheduler<P>
where
    P: Platform,
{
    pub running_task: Option<Arc<Task<P>>>,
    /// List of Tasks ready to be scheduled. Backed by a `VecDeque` so we can rotate objects in the queue efficiently.
    ready_queue: VecDeque<Arc<Task<P>>>,
    blocked_queue: Vec<Arc<Task<P>>>,
}

impl<P> CpuScheduler<P>
where
    P: Platform,
{
    pub fn new() -> CpuScheduler<P> {
        CpuScheduler { running_task: None, ready_queue: VecDeque::new(), blocked_queue: Vec::new() }
    }

    /// Choose the next task to be run. Returns `None` if no suitable task could be found to be run.
    fn choose_next(&mut self) -> Option<Arc<Task<P>>> {
        // TODO: in the future, this should consider task priorities etc.
        self.ready_queue.pop_front()
    }
}

impl<P> Scheduler<P>
where
    P: Platform,
{
    pub fn new() -> Scheduler<P> {
        Scheduler {
            task_scheduler: Spinlock::new(CpuScheduler::new()),
            tasklet_scheduler: TaskletScheduler::new(),
        }
    }

    pub fn add_task(&self, task: Arc<Task<P>>) {
        let mut scheduler = self.for_this_cpu();

        let current_state = task.state.lock().clone();
        match current_state {
            TaskState::Ready => scheduler.ready_queue.push_back(task),
            TaskState::Blocked(_) => scheduler.blocked_queue.push(task),
            TaskState::Running => panic!("Tried to schedule task that's already running!"),
        }
    }

    pub fn for_this_cpu(&self) -> SpinlockGuard<CpuScheduler<P>> {
        // XXX: this will need to take into account which CPU we're running on in the future
        self.task_scheduler.lock()
    }

    /// Start scheduling! This should be called after a platform has finished initializing, and is
    /// diverging. It gives kernel tasklets an initial poll while we're here in the kernel, and
    /// then drops down into userspace.
    pub fn start_scheduling(&self) -> ! {
        info!("Kernel initialization done. Dropping to userspace.");

        self.tasklet_scheduler.tick();

        let mut scheduler = self.for_this_cpu();
        assert!(scheduler.running_task.is_none());
        let task = scheduler.choose_next().expect("Tried to drop into userspace with no ready tasks!");
        assert!(task.state.lock().is_ready());
        Self::drop_to_userspace(scheduler, task);
    }

    /// Called when a userspace task yields or is pre-empted. This is responsible for the
    /// 'scheduling' part of the scheduler - it polls kernel tasklets as they need attention, and
    /// shares CPU time between userspace tasks.
    ///
    /// On each call to `schedule`, the kernel can choose to:
    ///    - Give CPU time to the kernel-space tasklet scheduler
    ///    - Switch to another userspace task
    ///    - Steal work from another CPU's scheduler
    ///    - Idle the CPU, if there is nothing to be done
    ///    - Nothing
    ///
    /// If the current task is switched away from, it will be placed in the state `new_state`. This
    /// allows the caller to block the current task on a dependency. If a task has been pre-empted
    /// or yields, it should be placed into `TaskState::Ready`.
    pub fn schedule(&self, new_state: TaskState) {
        self.tasklet_scheduler.tick();

        let mut scheduler = self.for_this_cpu();
        assert!(scheduler.running_task.is_some());
        if let Some(next_task) = scheduler.choose_next() {
            Self::switch_to(scheduler, new_state, next_task);
        } else {
            /*
             * There aren't any schedulable tasks. For now, we just return to the current one (by
             * doing nothing here).
             *
             * TODO: this should idle the CPU to minimise power use, waking to interrupts + every
             * so often to run tasklets, and see if any tasks are unblocked.
             */
            trace!("No more schedulable tasks. Returning to current one!");
        }
    }

    /// Perform the first transistion from the kernel into userspace. On some platforms, this has
    /// to be done differently to just a regular context-switch, so we handle it here separately.
    fn drop_to_userspace(mut scheduler: SpinlockGuard<CpuScheduler<P>>, task: Arc<Task<P>>) -> ! {
        trace!("Dropping into usermode into task: '{}'", task.name);

        *task.state.lock() = TaskState::Running;
        scheduler.running_task = Some(task.clone());
        task.address_space.switch_to();

        drop(scheduler);

        unsafe {
            let context = task.context.get() as *const P::TaskContext;
            P::drop_into_userspace(context)
        }
    }

    /// This actually performs a context switch between two tasks. It takes ownership of the locked
    /// `CpuScheduler` because we need to carefully release the lock before changing kernel stacks,
    /// else the next task will not be able to use the scheduler.
    ///
    /// This function returns when the userspace task that originally called `schedule` is
    /// scheduled again, as if nothing happened.
    fn switch_to(mut scheduler: SpinlockGuard<CpuScheduler<P>>, new_state: TaskState, next_task: Arc<Task<P>>) {
        /*
         * We're switching task! We sort out the internal scheduler state, and then ask the
         * platform to perform the context switch for us!
         * NOTE: This temporarily allows `running_task` to be `None`.
         */
        let current_task = scheduler.running_task.take().unwrap();
        assert!(current_task.state.lock().is_running());
        assert!(next_task.state.lock().is_ready());

        trace!("Switching from task '{}' to task '{}'", current_task.name, next_task.name);

        scheduler.running_task = Some(next_task.clone());
        *scheduler.running_task.as_ref().unwrap().state.lock() = TaskState::Running;
        match new_state {
            TaskState::Running => panic!("Tried to switch away from a task to state of Running!"),
            TaskState::Ready => {
                *current_task.state.lock() = TaskState::Ready;
                scheduler.ready_queue.push_back(current_task.clone());
            }
            TaskState::Blocked(block) => {
                trace!("Blocking task: {}", current_task.name);
                *current_task.state.lock() = TaskState::Blocked(block);
                scheduler.blocked_queue.push(current_task.clone());
            }
        }

        current_task.address_space.switch_from();
        next_task.address_space.switch_to();

        let from_context = current_task.context.get();
        let to_context = scheduler.running_task.as_ref().unwrap().context.get() as *const P::TaskContext;

        drop(scheduler);

        unsafe {
            P::context_switch(from_context, to_context);
        }
    }
}
