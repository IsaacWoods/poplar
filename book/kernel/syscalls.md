# System calls
Userspace code can interact with the kernel through system calls. Pebble's system call interface is based around
'kernel objects', and so many of the system calls are to create, destroy, or modify the state of various types of
kernel object. Because of Pebble's microkernel design, many traditional system calls (e.g. `open`) are not present,
their functionality instead being provided by userspace.

Each system call has a unique number that is used to identify it. A system call can then take up to five
parameters, each a maximum in size of the system's register width. It can return a single value, also the size of
a register.

### Overview of system calls

| Number    | System call               | Description                                               |
|-----------|---------------------------|-----------------------------------------------------------|
| `0`       | `yield`                   | Yield to the kernel.                                      |
| `1`       | `early_log`               | Log a message. Designed to be used from early processes.  |
| `2`       | `request_system_object`   | Request the id of a system kernel object.                 |
| `3`       | `my_address_space`        | Get the id of the calling task's AddressSpace.            |
| `4`       | `create_memory_object`    | Create a MemoryObject kernel object.                      |
| `5`       | `map_memory_object`       | Map a MemoryObject into an AddressSpace.                  |
| `6`       | `create_mailbox`          | Create a Mailbox kernel object.                           |
| `7`       | `wait_for_mail`           | Block until the given Mailbox receives mail.              |

### Making a system call on x86_64
To make a system call on x86_64, populate these registers:

| `rdi`                 | `rsi` | `rdx` | `r10` | `r8`  | `r9`  |
|-----------------------|-------|-------|-------|-------|-------|
| System call number    | `a`   | `b`   | `c`   | `d`   | `e`   |

The only way in which these registers deviate from the x86_64 Sys-V ABI is that `c` is passed in `r10` instead
of `rcx`. This is because `rcx` is used by the `syscall` instruction, and so is not free.
You can then make the system call by executing `syscall`. Before the kernel returns to userspace, it will put the
result of the system call (if there is one) in `rax`. If a system call takes less than five parameters, the unused
parameter registers will be preserved across the system call.

### Returning kernel object IDs
Often, system calls need to return something of the form of the Rust type `Result<KernelObjectId, SomeErrorType>`.

There is a common pattern used to represent this in the single `usize`:
* Bits `0..16` contain the index of the object's ID, if the call succeeded
* Bits `16..32` contain the generation of the object's ID, if the call succeeded
* Bits `32..64` contain the status:
    - `0` means the system call succeeded, and bits `0..32` contain a valid object ID
    - `>0` means that the system call failed. The meaning of the status is system-call dependent.
