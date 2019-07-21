use crate::{
    arch::Architecture,
    object::{
        task::{CommonTask, TaskState},
        WrappedKernelObject,
    },
};
use alloc::collections::VecDeque;
use core::fmt;

pub struct Scheduler<A: Architecture> {
    pub running_task: Option<WrappedKernelObject<A>>,
    /// List of Tasks ready to be scheduled. Every kernel object in this list must be a Task.
    /// Backed by a `VecDeque` so we can rotate objects in the queue efficiently.
    ready_queue: VecDeque<WrappedKernelObject<A>>,
}

impl<A> Scheduler<A>
where
    A: Architecture,
{
    pub fn new() -> Scheduler<A> {
        Scheduler { running_task: None, ready_queue: VecDeque::new() }
    }

    pub fn add_task(&mut self, task_object: WrappedKernelObject<A>) -> Result<(), ScheduleError> {
        let state = task_object.object.task().ok_or(ScheduleError::KernelObjectNotATask)?.read().state();
        match state {
            TaskState::Ready => self.ready_queue.push_back(task_object),
            TaskState::Running => panic!("Tried to schedule task that's already running!"),
        }

        Ok(())
    }

    /// Performs the first transistion from the kernel into userspace. On some platforms, this has
    /// to be done in a different way to how we'd replace the currently running task if we'd
    /// yielded or pre-empted out of an existing userspace context, and so this is handled
    /// specially.
    ///
    /// The scheduler will always drop into userspace into the first task added to the ready queue.
    /// By controlling which Task is added first, the ecosystem can be sure that the correct Task
    /// is run first (whether the userspace layers take advantage of this is up to them - it would
    /// be more reliable to not depend on one process starting first, but this is an option).
    pub fn drop_to_userspace(&mut self, arch: &A) -> ! {
        let task = self.ready_queue.pop_front().expect("Tried to drop into userspace with no ready tasks!");
        arch.drop_to_userspace(task)
    }
}

#[derive(Debug)]
pub enum ScheduleError {
    /// Returned by `add_task` if you try to schedule a kernel object that is not a Task.
    KernelObjectNotATask,
}

impl<A> fmt::Debug for Scheduler<A>
where
    A: Architecture,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Scheduler(running = {:?}, ready = {:?})", self.running_task, self.ready_queue)
    }
}
