use super::{address_space::AddressSpace, KernelObjectId};
use alloc::{string::String, sync::Arc, vec::Vec};
use hal::{memory::VirtualAddress, Hal};
use libpebble::caps::Capability;
use spin::Mutex;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TaskBlock {}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TaskState {
    Ready,
    Running,
    Blocked(TaskBlock),
}

pub enum TaskCreationError {
    /// The task name is not valid UTF-8.
    InvalidName,
    /// The loader can only load tasks that have a name that can be encoded in 32 bytes of UTF-8. This one is too
    /// long (so probably means that something hasn't been loaded correctly).
    NameTooLong,
    /// The byte stream describing the capabilities of an image is invalid.
    InvalidCapabilityEncoding,
}

/// Represents the layout of a task's usermode or kernelmode stack. A slot is allocated (contiguous from
/// `slot_bottom` to `top`), but only a portion of it may initially be mapped into backing memory (contiguous from
/// `stack_bottom` to `top`). A stack can be grown by allocating more backing memory and moving `stack_bottom` down
/// towards `slot_bottom`.
pub struct TaskStack {
    pub top: VirtualAddress,
    pub slot_bottom: VirtualAddress,
    pub stack_bottom: VirtualAddress,
}

pub struct Task<H>
where
    H: Hal,
{
    id: KernelObjectId,
    // TODO: do tasks have owners?
    owner: KernelObjectId,
    pub name: String,
    pub address_space: Arc<AddressSpace<H>>,
    pub status: Mutex<TaskState>,
    pub capabilities: Vec<Capability>,
    pub user_stack: Mutex<TaskStack>,
    pub kernel_stack: Mutex<TaskStack>,
}

impl<H> Task<H>
where
    H: Hal,
{
    pub fn from_boot_info(
        address_space: Arc<AddressSpace<H>>,
        image: &hal::boot_info::LoadedImage,
    ) -> Result<Task<H>, TaskCreationError> {
        // TODO: create user stack
        // TODO: create kernel stack
        todo!()
    }
}

/// Decode a capability stream (as found in a task's image) into a set of capabilities as they're
/// represented in the kernel. For the format that's being decoded here, refer to the
/// `(3.1) Userspace/Capabilities` section of the Book.
// TODO: this shouldn't be here - decoding capabilities is arch-independent
fn decode_capabilities(mut cap_stream: &[u8]) -> Result<Vec<Capability>, TaskCreationError> {
    let mut caps = Vec::new();

    // TODO: when decl_macro hygiene-opt-out is implemented, this should be converted to use it
    macro_rules! one_byte_cap {
        ($cap: path) => {{
            caps.push($cap);
            cap_stream = &cap_stream[1..];
        }};
    }

    while cap_stream.len() > 0 {
        match cap_stream[0] {
            0x01 => one_byte_cap!(Capability::CreateAddressSpace),
            0x02 => one_byte_cap!(Capability::CreateMemoryObject),
            0x03 => one_byte_cap!(Capability::CreateTask),

            0x30 => one_byte_cap!(Capability::AccessBackupFramebuffer),
            0x31 => one_byte_cap!(Capability::EarlyLogging),

            // We skip `0x00` as the first byte of a capability, as it is just used to pad the
            // stream and so has no meaning
            0x00 => cap_stream = &cap_stream[1..],

            _ => return Err(TaskCreationError::InvalidCapabilityEncoding),
        }
    }

    Ok(caps)
}
