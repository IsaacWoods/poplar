use super::{
    address_space::{AddressSpace, TaskSlot},
    alloc_kernel_object_id,
    KernelObject,
    KernelObjectId,
};
use crate::{
    memory::{KernelStackAllocator, PhysicalMemoryManager, Stack},
    Platform,
};
use alloc::{collections::BTreeMap, string::String, sync::Arc, vec::Vec};
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicU32, Ordering},
};
use hal::memory::VirtualAddress;
use libpebble::{caps::Capability, Handle};
use spin::{Mutex, RwLock};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TaskBlock {}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TaskState {
    Ready,
    Running,
    Blocked(TaskBlock),
}

#[derive(Debug)]
pub enum TaskCreationError {
    /// The task name is not valid UTF-8.
    InvalidName,
    /// The loader can only load tasks that have a name that can be encoded in 32 bytes of UTF-8. This one is too
    /// long (so probably means that something hasn't been loaded correctly).
    NameTooLong,
    /// The byte stream describing the capabilities of an image is invalid.
    InvalidCapabilityEncoding,
    /// The `AddressSpace` that this task has been created in cannot contain any more tasks.
    AddressSpaceFull,
    /// The kernel stack allocator has run out of slots - this means too many tasks have been started.
    NoKernelStackSlots,
}

pub struct Task<P>
where
    P: Platform,
{
    id: KernelObjectId,
    owner: KernelObjectId,
    pub name: String,
    pub address_space: Arc<AddressSpace<P>>,
    pub state: Mutex<TaskState>,
    pub capabilities: Vec<Capability>,

    pub user_slot: Mutex<TaskSlot>,
    pub kernel_stack: Mutex<Stack>,
    pub kernel_stack_pointer: UnsafeCell<VirtualAddress>,
    pub user_stack_pointer: UnsafeCell<VirtualAddress>,

    pub handles: RwLock<BTreeMap<Handle, Arc<dyn KernelObject>>>,
    next_handle: AtomicU32,
}

/*
 * XXX: this is needed to make `Task` Sync because there's that UnsafeCell in there. We should actually have
 * some sort of synchronization primitive that says "only this scheduler can access me" instead (I think) and
 * then unsafe impl these traits on that instead.
 */
unsafe impl<P> Send for Task<P> where P: Platform {}
unsafe impl<P> Sync for Task<P> where P: Platform {}

impl<P> Task<P>
where
    P: Platform,
{
    pub fn from_boot_info(
        owner: KernelObjectId,
        address_space: Arc<AddressSpace<P>>,
        image: &hal::boot_info::LoadedImage,
        allocator: &PhysicalMemoryManager,
        kernel_page_table: &mut P::PageTable,
        kernel_stack_allocator: &mut KernelStackAllocator<P>,
    ) -> Result<Arc<Task<P>>, TaskCreationError> {
        // TODO: better way of getting initial stack sizes
        let task_slot =
            address_space.alloc_task_slot(0x4000, allocator).ok_or(TaskCreationError::AddressSpaceFull)?;
        let kernel_stack = kernel_stack_allocator
            .alloc_kernel_stack(0x4000, allocator, kernel_page_table)
            .ok_or(TaskCreationError::NoKernelStackSlots)?;

        let mut kernel_stack_pointer = kernel_stack.top;
        let mut user_stack_pointer = task_slot.user_stack.top;
        unsafe {
            P::initialize_task_kernel_stack(&mut kernel_stack_pointer, image.entry_point, &mut user_stack_pointer);
        }

        Ok(Arc::new(Task {
            id: alloc_kernel_object_id(),
            owner,
            name: String::from(image.name()),
            address_space,
            state: Mutex::new(TaskState::Ready),
            capabilities: decode_capabilities(&image.capability_stream)?,
            user_slot: Mutex::new(task_slot),
            kernel_stack: Mutex::new(kernel_stack),
            kernel_stack_pointer: UnsafeCell::new(kernel_stack_pointer),
            user_stack_pointer: UnsafeCell::new(user_stack_pointer),
            handles: RwLock::new(BTreeMap::new()),
            // XXX: 0 is a special handle value, so start at 1
            next_handle: AtomicU32::new(1),
        }))
    }

    pub fn add_handle(&self, object: Arc<dyn KernelObject>) -> Handle {
        let handle_num = self.next_handle.fetch_add(1, Ordering::Relaxed);
        self.handles.write().insert(Handle(handle_num), object);
        Handle(handle_num)
    }
}

impl<P> KernelObject for Task<P>
where
    P: Platform,
{
    fn id(&self) -> KernelObjectId {
        self.id
    }
}

/// Decode a capability stream (as found in a task's image) into a set of capabilities as they're
/// represented in the kernel. For the format that's being decoded here, refer to the
/// `(3.1) Userspace/Capabilities` section of the Book.
fn decode_capabilities(mut cap_stream: &[u8]) -> Result<Vec<Capability>, TaskCreationError> {
    use libpebble::caps::*;

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
            CAP_GET_FRAMEBUFFER => one_byte_cap!(Capability::GetFramebuffer),
            CAP_EARLY_LOGGING => one_byte_cap!(Capability::EarlyLogging),
            CAP_SERVICE_PROVIDER => one_byte_cap!(Capability::ServiceProvider),
            CAP_SERVICE_USER => one_byte_cap!(Capability::ServiceUser),
            CAP_PCI_BUS_DRIVER => one_byte_cap!(Capability::PciBusDriver),

            // We skip `0x00` as the first byte of a capability, as it is just used to pad the
            // stream and so has no meaning
            0x00 => cap_stream = &cap_stream[1..],

            _ => return Err(TaskCreationError::InvalidCapabilityEncoding),
        }
    }

    Ok(caps)
}
