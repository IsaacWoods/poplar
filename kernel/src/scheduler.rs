use crate::{
    arch::Architecture,
    object::{
        task::{CommonTask, TaskState},
        WrappedKernelObject,
    },
};
use alloc::collections::VecDeque;
use core::fmt;
use log::trace;

pub struct Scheduler {
    pub running_task: Option<WrappedKernelObject<crate::arch_impl::Arch>>,
    /// List of Tasks ready to be scheduled. Every kernel object in this list must be a Task.
    /// Backed by a `VecDeque` so we can rotate objects in the queue efficiently.
    ready_queue: VecDeque<WrappedKernelObject<crate::arch_impl::Arch>>,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler { running_task: None, ready_queue: VecDeque::new() }
    }

    pub fn add_task(
        &mut self,
        task_object: WrappedKernelObject<crate::arch_impl::Arch>,
    ) -> Result<(), ScheduleError> {
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
    pub fn drop_to_userspace(&mut self, arch: &crate::arch_impl::Arch) -> ! {
        assert!(self.running_task.is_none());
        let task = self.ready_queue.pop_front().expect("Tried to drop into userspace with no ready tasks!");
        self.running_task = Some(task.clone());
        arch.drop_to_userspace(task)
    }

    /// Switch to the next scheduled task. This is called when a task yields, or when we pre-empt a
    /// task that is hogging CPU time. If there is nothing to schedule, this is free to idle the
    /// CPU (including managing power), or steal work from another scheduling unit.
    pub fn switch_to_next(&mut self) {
        assert!(self.running_task.is_some());

        /*
         * Select the next task to run.
         * NOTE: in the future, this could be more complex, e.g. by taking priority into account.
         */
        if let Some(next_task) = self.ready_queue.pop_front() {
            /*
             * We're switching task! We sort out the internal scheduler state, and then ask the
             * platform to perform the context switch for us!
             * NOTE: This temporarily allows `running_task` to be `None`.
             */
            trace!("switching task: {}", next_task.object.task().unwrap().read().name());
            let old_task = self.running_task.take().unwrap();
            self.running_task = Some(next_task.clone());
            self.ready_queue.push_back(old_task.clone());

            /*
             * On some platforms, this may not always return, and so we must not be holding any
             * locks when we call this (this is why it takes the kernel objects directly).
             */
            crate::arch_impl::context_switch(old_task, next_task);
        } else {
            /*
             * There aren't any schedulable tasks. For now, we just return to the current one (by
             * doing nothing here).
             */
            trace!("No more schedulable tasks. Returning to current one!");
        }
    }
}

#[derive(Debug)]
pub enum ScheduleError {
    /// Returned by `add_task` if you try to schedule a kernel object that is not a Task.
    KernelObjectNotATask,
}

impl fmt::Debug for Scheduler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Scheduler(running = {:?}, ready = {:?})", self.running_task, self.ready_queue)
    }
}
