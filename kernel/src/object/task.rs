use super::{address_space::AddressSpace, alloc_kernel_object_id, KernelObject, KernelObjectId};
use crate::{memory::PhysicalMemoryManager, per_cpu::KernelPerCpu, slab_allocator::SlabAllocator};
use alloc::{string::String, sync::Arc, vec::Vec};
use hal::{memory::VirtualAddress, Hal};
use libpebble::caps::Capability;
use spin::Mutex;

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
    H: Hal<KernelPerCpu>,
{
    id: KernelObjectId,
    // TODO: do tasks have owners?
    owner: KernelObjectId,
    pub name: String,
    pub address_space: Arc<AddressSpace<H>>,
    pub state: Mutex<TaskState>,
    pub capabilities: Vec<Capability>,
    pub user_stack: Mutex<TaskStack>,
    pub kernel_stack: Mutex<TaskStack>,

    pub kernel_stack_pointer: Mutex<VirtualAddress>,
}

impl<H> Task<H>
where
    H: Hal<KernelPerCpu>,
{
    pub fn from_boot_info(
        owner: KernelObjectId,
        address_space: Arc<AddressSpace<H>>,
        image: &hal::boot_info::LoadedImage,
        allocator: &PhysicalMemoryManager<H>,
        kernel_page_table: &mut H::PageTable,
        kernel_stack_allocator: &mut KernelStackAllocator,
    ) -> Result<Arc<Task<H>>, TaskCreationError> {
        use hal::TaskHelper;

        // TODO: better way of getting initial stack sizes
        let user_stack =
            address_space.alloc_user_stack(0x1000, allocator).ok_or(TaskCreationError::AddressSpaceFull)?;
        let kernel_stack = kernel_stack_allocator
            .alloc_kernel_task_stack(0x1000, allocator, kernel_page_table)
            .ok_or(TaskCreationError::NoKernelStackSlots)?;

        let mut kernel_stack_pointer = kernel_stack.top;
        unsafe {
            H::TaskHelper::initialize_kernel_stack(&mut kernel_stack_pointer, image.entry_point, user_stack.top);
        }

        Ok(Arc::new(Task {
            id: alloc_kernel_object_id(),
            owner,
            name: String::from(image.name()),
            address_space,
            state: Mutex::new(TaskState::Ready),
            capabilities: decode_capabilities(&image.capability_stream)?,
            user_stack: Mutex::new(user_stack),
            kernel_stack: Mutex::new(kernel_stack),
            kernel_stack_pointer: Mutex::new(kernel_stack_pointer),
        }))
    }
}

impl<H> KernelObject for Task<H>
where
    H: Hal<KernelPerCpu>,
{
    fn id(&self) -> KernelObjectId {
        self.id
    }
}

/// Decode a capability stream (as found in a task's image) into a set of capabilities as they're
/// represented in the kernel. For the format that's being decoded here, refer to the
/// `(3.1) Userspace/Capabilities` section of the Book.
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

pub struct KernelStackAllocator {
    kernel_stack_slots: Mutex<SlabAllocator>,
    slot_size: usize,
}

impl KernelStackAllocator {
    pub fn new(
        stacks_bottom: VirtualAddress,
        stacks_top: VirtualAddress,
        slot_size: usize,
    ) -> KernelStackAllocator {
        KernelStackAllocator {
            kernel_stack_slots: Mutex::new(SlabAllocator::new(stacks_bottom, stacks_top, slot_size)),
            slot_size,
        }
    }

    pub fn alloc_kernel_task_stack<H>(
        &self,
        initial_size: usize,
        physical_memory_manager: &PhysicalMemoryManager<H>,
        kernel_page_table: &mut H::PageTable,
    ) -> Option<TaskStack>
    where
        H: Hal<KernelPerCpu>,
    {
        use hal::memory::{Flags, PageTable};

        let slot_bottom = self.kernel_stack_slots.lock().alloc()?;
        let top = slot_bottom + self.slot_size - 1;
        let stack_bottom = top - initial_size + 1;

        let physical_start = physical_memory_manager.alloc_bytes(initial_size);
        kernel_page_table
            .map_area(
                stack_bottom,
                physical_start,
                initial_size,
                Flags { writable: true, user_accessible: true, ..Default::default() },
                physical_memory_manager,
            )
            .unwrap();

        Some(TaskStack { top, slot_bottom, stack_bottom })
    }
}
