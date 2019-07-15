use super::{memory::userspace_map, per_cpu, Arch};
use crate::object::{
    task::{CommonTask, TaskState},
    WrappedKernelObject,
};
use core::pin::Pin;
use x86_64::{hw::tss::Tss, memory::VirtualAddress};

/// This is the representation of a task on x86_64. It's basically just keeps information about the
/// current instruction pointer and the stack, as all other registers are preserved on the task
/// stack when it's suspended.
pub struct Task {
    pub address_space: WrappedKernelObject<Arch>,
    pub state: TaskState,

    pub stack_top: VirtualAddress,
    pub stack_size: usize,

    pub instruction_pointer: VirtualAddress,
    pub stack_pointer: VirtualAddress,
}

impl Task {
    /// Create a new task in a given address space, which will start executing at the given entry
    /// point when scheduled. This creates a new userspace stack in the given address space.
    ///
    /// ### Panics
    /// * If the given address space doesn't point to a valid `AddressSpace`
    /// * If the `AddressSpace` fails to create a new stack for the task
    // TODO: in the future, this should handle errors better than causing kernel panics because
    // this is likely to be called from userspace (through syscalls) - although we could push
    // validation of that to the syscall code
    pub fn new(arch: &Arch, address_space: WrappedKernelObject<Arch>, entry_point: VirtualAddress) -> Task {
        let stack_set = address_space
            .object
            .address_space()
            .expect("Not an address space")
            .write()
            .add_stack_set(userspace_map::INITIAL_STACK_SIZE, &arch.physical_memory_manager)
            .expect("Failed to allocate stacks for task");

        Task {
            address_space,
            state: TaskState::Ready,
            stack_top: stack_set.user_stack_top,
            stack_size: userspace_map::INITIAL_STACK_SIZE,
            instruction_pointer: entry_point,
            stack_pointer: stack_set.user_stack_top,
        }
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

pub fn drop_to_usermode(arch: &Arch, task: WrappedKernelObject<Arch>) -> ! {
    use x86_64::hw::gdt::{USER_CODE64_SELECTOR, USER_DATA_SELECTOR};

    unsafe {
        /*
         * Disable interrupts so we aren't interrupted in the middle of this. They are
         * re-enabled on the `iret`.
         */
        asm!("cli");

        /*
         * Save the current kernel stack pointer in the TSS.
         */
        let rsp: VirtualAddress;
        asm!(""
         : "={rsp}"(rsp)
         :
         : "rsp"
         : "intel"
        );

        // Safe because we don't move the TSS by changing the kernel stack
        Pin::get_unchecked_mut(per_cpu::per_cpu_data_mut().get_tss_mut()).set_kernel_stack(rsp);

        /*
         * Switch to the address space the task resides in, and extract the information we need
         * to start executing the task. We do this in advance to make sure we end the
         * locks on everything - if we don't do this in its own scope, they'd never get
         * dropped, because this function never returns.
         */
        let (entry_point, stack_pointer) = {
            let task_object = task.object.task().expect("Not a task").write();
            task_object.address_space.object.address_space().unwrap().write().switch_to();
            (task_object.instruction_pointer, task_object.stack_pointer)
        };

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
          "{rbx}"(stack_pointer),
          "{rcx}"(1<<9 | 1<<2),
          "{rdx}"(USER_CODE64_SELECTOR),
          "{rsi}"(entry_point)
        : "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "r8", "r9", "r10", "r11", "r12", "r13", "r14", "r15"
        : "intel"
        );
        unreachable!();
    }
}
