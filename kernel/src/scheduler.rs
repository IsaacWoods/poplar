use crate::{
    object::task::{Task, TaskState},
    per_cpu::PerCpu,
    Platform,
};
use alloc::{collections::VecDeque, sync::Arc, vec::Vec};
use hal::memory::VAddr;
use tracing::trace;

pub struct Scheduler<P>
where
    P: Platform,
{
    pub running_task: Option<Arc<Task<P>>>,
    /// List of Tasks ready to be scheduled. Every kernel object in this list must be a Task.
    /// Backed by a `VecDeque` so we can rotate objects in the queue efficiently.
    ready_queue: VecDeque<Arc<Task<P>>>,
    blocked_queue: Vec<Arc<Task<P>>>,
}

impl<P> Scheduler<P>
where
    P: Platform,
{
    pub fn new() -> Scheduler<P> {
        Scheduler { running_task: None, ready_queue: VecDeque::new(), blocked_queue: Vec::new() }
    }

    pub fn add_task(&mut self, task: Arc<Task<P>>) {
        let current_state = task.state.lock().clone();
        match current_state {
            TaskState::Ready => self.ready_queue.push_back(task),
            TaskState::Blocked(_) => self.blocked_queue.push(task),
            TaskState::Running => panic!("Tried to schedule task that's already running!"),
        }
    }

    /// Choose the next task to be run. Returns `None` if no suitable task could be found to be run.
    fn choose_next(&mut self) -> Option<Arc<Task<P>>> {
        // TODO: in the future, this should consider task priorities etc.
        self.ready_queue.pop_front()
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
    pub fn drop_to_userspace(&mut self) -> ! {
        assert!(self.running_task.is_none());
        let task = self.ready_queue.pop_front().expect("Tried to drop into userspace with no ready tasks!");
        assert_eq!(*task.state.lock(), TaskState::Ready);

        trace!("Dropping into usermode into task: '{}'", task.name);

        *task.state.lock() = TaskState::Running;
        self.running_task = Some(task.clone());
        task.address_space.switch_to();

        unsafe {
            let kernel_stack_pointer: VAddr = *task.kernel_stack_pointer.get();
            let user_stack_pointer: VAddr = *task.user_stack_pointer.get();
            trace!("Setting stacks - kernel: {:#x}, user: {:#x}", kernel_stack_pointer, user_stack_pointer);

            P::per_cpu().set_kernel_stack_pointer(kernel_stack_pointer);
            P::per_cpu().set_user_stack_pointer(user_stack_pointer);
            P::drop_into_userspace()
        }
    }

    /// Switch to the next scheduled task. This is called when a task yields, or when we pre-empt a
    /// task that is hogging CPU time. If there is nothing to schedule, this is free to idle the
    /// CPU (including managing power), or steal work from another scheduling unit.
    ///
    /// The task being switched away from is moved to state `new_state` (this allows you to block the current task.
    /// If it's just being preempted or has yielded, use `TaskState::Ready`).
    pub fn switch_to_next(&mut self, new_state: TaskState) {
        assert!(self.running_task.is_some());

        if let Some(next_task) = self.choose_next() {
            /*
             * We're switching task! We sort out the internal scheduler state, and then ask the
             * platform to perform the context switch for us!
             * NOTE: This temporarily allows `running_task` to be `None`.
             */
            trace!("Switching to task: {}", next_task.name);
            let old_task = self.running_task.take().unwrap();
            assert_eq!(*old_task.state.lock(), TaskState::Running);
            assert_eq!(*next_task.state.lock(), TaskState::Ready);

            self.running_task = Some(next_task.clone());
            *self.running_task.as_ref().unwrap().state.lock() = TaskState::Running;
            match new_state {
                TaskState::Running => panic!("Tried to switch away from a task to state of Running!"),
                TaskState::Ready => {
                    *old_task.state.lock() = TaskState::Ready;
                    self.ready_queue.push_back(old_task.clone());
                }
                TaskState::Blocked(block) => {
                    trace!("Blocking task: {}", old_task.name);
                    *old_task.state.lock() = TaskState::Blocked(block);
                    self.blocked_queue.push(old_task.clone());
                }
            }

            old_task.address_space.switch_from();
            next_task.address_space.switch_to();

            let old_kernel_stack: *mut VAddr = old_task.kernel_stack_pointer.get();
            let new_kernel_stack = unsafe { *self.running_task.as_ref().unwrap().kernel_stack_pointer.get() };
            let new_user_stack = unsafe { *self.running_task.as_ref().unwrap().user_stack_pointer.get() };

            unsafe {
                *old_task.user_stack_pointer.get() = P::per_cpu().user_stack_pointer();
                trace!("Setting stacks - kernel: {:#x}, user: {:#x}", new_kernel_stack, new_user_stack);
                P::per_cpu().set_kernel_stack_pointer(new_kernel_stack);
                P::per_cpu().set_user_stack_pointer(new_user_stack);
                P::context_switch(old_kernel_stack, new_kernel_stack);
            }
        } else {
            /*
             * There aren't any schedulable tasks. For now, we just return to the current one (by
             * doing nothing here).
             * TODO: this should catch up on any kernel bookkeeping, then idle to minimise power use.
             */
            trace!("No more schedulable tasks. Returning to current one!");
        }
    }
}
