use super::{
    address_space::{AddressSpace, TaskSlot},
    alloc_kernel_object_id,
    event::Event,
    KernelObject,
    KernelObjectId,
    KernelObjectType,
};
use crate::{
    memory::{vmm::Stack, Pmm},
    Platform,
};
use alloc::{collections::BTreeMap, string::String, sync::Arc};
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicU32, Ordering},
};
use hal::memory::VAddr;
use poplar::Handle;
use spinning_top::{RwSpinlock, Spinlock};

#[derive(Clone, Debug)]
pub enum TaskBlock {
    OnEvent(Arc<Event>),
}

#[derive(Clone, Debug)]
pub enum TaskState {
    Ready,
    Running,
    Blocked(TaskBlock),
}

impl TaskState {
    pub fn is_ready(&self) -> bool {
        match self {
            TaskState::Ready => true,
            _ => false,
        }
    }

    pub fn is_running(&self) -> bool {
        match self {
            TaskState::Running => true,
            _ => false,
        }
    }

    pub fn is_blocked(&self) -> bool {
        match self {
            TaskState::Blocked(_) => true,
            _ => false,
        }
    }
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
    pub state: Spinlock<TaskState>,

    pub user_slot: Spinlock<TaskSlot>,
    pub kernel_stack: Spinlock<Stack>,
    pub kernel_stack_pointer: UnsafeCell<VAddr>,
    pub user_stack_pointer: UnsafeCell<VAddr>,

    pub context: UnsafeCell<P::TaskContext>,

    pub handles: Handles,
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
    pub fn new(
        owner: KernelObjectId,
        address_space: Arc<AddressSpace<P>>,
        name: String,
        entry_point: VAddr,
        handles: Handles,
        allocator: &Pmm,
        kernel_page_table: &mut P::PageTable,
    ) -> Result<Arc<Task<P>>, TaskCreationError> {
        let id = alloc_kernel_object_id();

        // TODO: better way of getting initial stack sizes
        let task_slot =
            address_space.alloc_task_slot(0x8000, allocator).ok_or(TaskCreationError::AddressSpaceFull)?;
        let kernel_stack = crate::VMM
            .get()
            .alloc_kernel_stack::<P>(0x4000, allocator, kernel_page_table)
            .ok_or(TaskCreationError::NoKernelStackSlots)?;

        let (kernel_stack_pointer, user_stack_pointer) =
            unsafe { P::initialize_task_stacks(&kernel_stack, &task_slot.user_stack, entry_point) };
        let context = P::new_task_context(kernel_stack_pointer, user_stack_pointer, entry_point);

        Ok(Arc::new(Task {
            id,
            owner,
            name,
            address_space,
            state: Spinlock::new(TaskState::Ready),
            user_slot: Spinlock::new(task_slot),
            kernel_stack: Spinlock::new(kernel_stack),
            kernel_stack_pointer: UnsafeCell::new(kernel_stack_pointer),
            user_stack_pointer: UnsafeCell::new(user_stack_pointer),

            context: UnsafeCell::new(context),

            handles,
        }))
    }
}

impl<P> KernelObject for Task<P>
where
    P: Platform,
{
    fn id(&self) -> KernelObjectId {
        self.id
    }

    fn typ(&self) -> KernelObjectType {
        KernelObjectType::Task
    }
}

pub struct Handles {
    handles: RwSpinlock<BTreeMap<Handle, Arc<dyn KernelObject>>>,
    next: AtomicU32,
}

impl Handles {
    pub fn new() -> Handles {
        Handles {
            handles: RwSpinlock::new(BTreeMap::new()),
            // XXX: 0 is a special handle value, so start at 1
            next: AtomicU32::new(1),
        }
    }

    pub fn add(&self, object: Arc<dyn KernelObject>) -> Handle {
        let handle_num = self.next.fetch_add(1, Ordering::Relaxed);
        self.handles.write().insert(Handle(handle_num), object);
        Handle(handle_num)
    }

    pub fn remove(&self, handle: Handle) {
        self.handles.write().remove(&handle);
    }

    pub fn get(&self, handle: Handle) -> Option<Arc<dyn KernelObject>> {
        self.handles.read().get(&handle).cloned()
    }
}
