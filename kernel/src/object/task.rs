#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TaskState {
    Ready,
    Running,
}

/// Implemented by each of the structures that provide the platform-specific Task implementations.
/// Allows the platform-independent parts of the kernel (e.g. scheduler) to work with Tasks.
pub trait CommonTask {
    fn state(&self) -> TaskState;

    fn name(&self) -> &str;
    fn switch_to(&mut self);
}
