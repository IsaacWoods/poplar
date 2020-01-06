//! This module contains types that can be re-used between common parts of the kernel and all the
//! architecture modules.

use libpebble::KernelObjectId;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TaskBlock {
    /// Block the task until the mailbox with the given ID receives mail
    WaitForMail(KernelObjectId),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TaskState {
    Ready,
    Running,
    Blocked(TaskBlock),
}

/// Implemented by each of the structures that provide the platform-specific Task implementations.
/// Allows the platform-independent parts of the kernel (e.g. scheduler) to work with Tasks.
pub trait CommonTask {
    fn state(&self) -> TaskState;
    fn name(&self) -> &str;
}
