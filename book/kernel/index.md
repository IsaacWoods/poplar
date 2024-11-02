# The Poplar Kernel
At the core of Poplar is a small Rust microkernel. The kernel's job is to multiplex access to hardware resources
(such as CPU time, memory, and peripherals) between competing bits of userspace code.

Poplar is a microkernel because its drivers, pieces of code for managing the many different devices a computer
may have, live in userspace, and are relatively unpriviledged compared to the kernel. This provides safety benefits
over a monolithic kernel because a misbehaving or malicious driver (supplied by a hardware vendor, for example) has
a much more limited scope of operation. The disadvantage is that microkernels tend to be slower than monolithic
kernels, due to increased overheads of communication between the kernel and userspace.

## Kernel objects
The Poplar microkernel is object-based - resources managed by the kernel are represented as discrete 'objects', and
are interacted with from userspace via plain integers called 'handles'. Multiple handles referring to a single object
can exist, and each possesses a set of permissions that dictate how the owning task can interact with the object.

Kernel objects are used for:
- Task creation and management (e.g. `AddressSpace` and `Task`)
- Access to hardware resources (e.g. `MemoryObject`)
- Message passing between tasks (e.g. `Channel`)
- Signaling and waiting (e.g. `Event`)