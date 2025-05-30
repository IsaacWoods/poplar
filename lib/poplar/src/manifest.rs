//! Each task is passed a 'manifest' when it is started that details the handles the task has been
//! created with, boot arguments, etc. This is encoded using Ptah.

use alloc::{string::String, vec::Vec};
use ptah::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BootstrapManifest {
    pub task_name: String,
    pub boot_tasks: Vec<BootTask>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BootTask {
    pub name: String,
    pub entry_point: usize,
    /// The segments that should be loaded into the task's address space. In the format `(virtual
    /// address, handle to MemoryObject)`.
    pub segments: Vec<(usize, u32)>,
}
