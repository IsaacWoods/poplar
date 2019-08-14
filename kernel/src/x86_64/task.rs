use super::{memory::userspace_map, per_cpu, Arch};
use crate::object::{
    task::{CommonTask, TaskState},
    WrappedKernelObject,
};
use alloc::vec::Vec;
use core::pin::Pin;
use libpebble::caps::Capability;
use x86_64::{boot::ImageInfo, memory::VirtualAddress};

#[derive(Debug)]
pub enum TaskCreationError {
    /// The kernel object that should be the AddressSpace that this task will live in is not
    /// actually an AddressSpace
    NotAnAddressSpace,

    /// The AddressSpace has run out of stack slots
    NotEnoughStackSlots,

    /// The task image has an invalid capability encoding
    InvalidCapabilityEncoding,
}

/// This is the representation of a task on x86_64. It's basically just keeps information about the
/// current instruction pointer and the stack, as all other registers are preserved on the task
/// stack when it's suspended.
pub struct Task {
    pub address_space: WrappedKernelObject<Arch>,
    pub state: TaskState,
    pub capabilities: Vec<Capability>,

    pub user_stack_top: VirtualAddress,
    pub kernel_stack_top: VirtualAddress,
    pub stack_size: usize,

    pub instruction_pointer: VirtualAddress,

    /*
     * We only keep track of the kernel stack pointer. The user stack pointer is saved on the
     * kernel stack when we enter the kernel through the `syscall` instruction, and restored before
     * a `sysret`.
     */
    pub kernel_stack_pointer: VirtualAddress,
}

impl Task {
    /// Create a new task in a given address space, which will start executing at the given entry
    /// point when scheduled. This creates a new userspace stack in the given address space.
    ///
    /// ### Panics
    /// * If the given address space doesn't point to a valid `AddressSpace`
    /// * If the `AddressSpace` fails to create a new stack for the task
    pub fn from_image_info(
        arch: &Arch,
        address_space: WrappedKernelObject<Arch>,
        image: &ImageInfo,
    ) -> Result<Task, TaskCreationError> {
        let stack_set = address_space
            .object
            .address_space()
            .ok_or(TaskCreationError::NotAnAddressSpace)?
            .write()
            .add_stack_set(userspace_map::INITIAL_STACK_SIZE, &arch.physical_memory_manager)
            .ok_or(TaskCreationError::NotEnoughStackSlots)?;

        Ok(Task {
            address_space,
            state: TaskState::Ready,
            capabilities: decode_capabilities(&image.capability_stream)?,
            user_stack_top: stack_set.user_stack_top,
            kernel_stack_top: stack_set.kernel_stack_top,
            stack_size: userspace_map::INITIAL_STACK_SIZE,
            instruction_pointer: image.entry_point,
            kernel_stack_pointer: stack_set.kernel_stack_top,
        })
    }
}

impl CommonTask for Task {
    fn state(&self) -> TaskState {
        self.state
    }

    fn switch_to(&mut self) {
        assert_eq!(self.state, TaskState::Ready);
        self.address_space.object.address_space().unwrap().write().switch_to();
        self.state = TaskState::Running;
    }
}

/// Drop into usermode into the given task. This permanently migrates from the kernel's initial
/// stack (the one reserved in `.bss` and used during kernel initialization).
pub fn drop_to_usermode(arch: &Arch, task: WrappedKernelObject<Arch>) -> ! {
    use x86_64::hw::gdt::{USER_CODE64_SELECTOR, USER_DATA_SELECTOR};

    unsafe {
        /*
         * Disable interrupts so we aren't interrupted in the middle of this. They are
         * re-enabled on the `iret`.
         */
        asm!("cli");

        /*
         * Switch to the address space the task resides in, and extract the information we need
         * to start executing the task. We do this in advance to make sure we end the
         * locks on everything - if we don't do this in its own scope, they'd never get
         * dropped, because this function never returns.
         *
         * We can just take the tops of each stack, because we can only drop to userspace once,
         * before any tasks have executed anything, so nothing will be on their stacks.
         */
        let (entry_point, user_stack_top, kernel_stack_top) = {
            let mut task_object = task.object.task().expect("Not a task").write();
            task_object.address_space.object.address_space().unwrap().write().switch_to();
            task_object.state = TaskState::Running;
            (task_object.instruction_pointer, task_object.user_stack_top, task_object.kernel_stack_top)
        };

        /*
         * Set the kernel stack in the TSS to the task's kernel stack. This is safe because
         * changing the kernel stack does not move the TSS.
         */
        Pin::get_unchecked_mut(per_cpu::per_cpu_data_mut().get_tss_mut()).set_kernel_stack(kernel_stack_top);
        *Pin::get_unchecked_mut(per_cpu::per_cpu_data_mut().current_task_kernel_rsp_mut()) = kernel_stack_top;

        /*
         * Enter Ring 3 by constructing a fake interrupt frame, then returning from the
         * "interrupt".
         */
        asm!("// Push selector for user data segment
              push rax

              // Push new stack pointer
              push rbx

              // Push new RFLAGS. We set this to the bare minimum to avoid leaking flags out of the
              // kernel. Bit 2 must be one, and we enable interrupts by setting bit 9.
              push rcx

              // Push selector for user code segment
              push rdx

              // Push new instruction pointer
              push rsi

              // Zero all the things
              xor rax, rax
              xor rbx, rbx
              xor rcx, rcx
              xor rdx, rdx
              xor rsi, rsi
              xor rdi, rdi
              xor r8, r8
              xor r9, r9
              xor r10, r10
              xor r11, r11
              xor r12, r12
              xor r13, r13
              xor r14, r14
              xor r15, r15

              // Return from our fake interrupt frame
              iretq
              "
        :
        : "{rax}"(USER_DATA_SELECTOR),
          "{rbx}"(user_stack_top),
          "{rcx}"(1<<9 | 1<<2),
          "{rdx}"(USER_CODE64_SELECTOR),
          "{rsi}"(entry_point)
        : "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "r8", "r9", "r10", "r11", "r12", "r13", "r14", "r15"
        : "intel"
        );
        unreachable!();
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

            0x30 => one_byte_cap!(Capability::MapFramebuffer),
            0x31 => one_byte_cap!(Capability::EarlyLogging),

            // We skip `0x00` as the first byte of a capability, as it is just used to pad the
            // stream and so has no meaning
            0x00 => cap_stream = &cap_stream[1..],

            _ => return Err(TaskCreationError::InvalidCapabilityEncoding),
        }
    }

    Ok(caps)
}
