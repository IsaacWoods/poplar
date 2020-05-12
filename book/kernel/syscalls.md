# System calls
Userspace code can interact with the kernel through system calls. Pebble's system call interface is based around
'kernel objects', and so many of the system calls are to create, destroy, or modify the state of various types of
kernel object. Because of Pebble's microkernel design, many traditional system calls (e.g. `open`) are not present,
their functionality instead being provided by userspace.

Each system call has a unique number that is used to identify it. A system call can then take up to five
parameters, each a maximum in size of the system's register width. It can return a single value, also the size of
a register.

### Overview of system calls

| Number    | System call               | Description                                                     |
|-----------|---------------------------|-----------------------------------------------------------------|
| `0`       | `yield`                   | Yield to the kernel.                                            |
| `1`       | `early_log`               | Log a message. Designed to be used from early processes.        |
| `2`       | `get_framebuffer`         | Get the framebuffer that the kernel has created, if it has.     |
| `3`       | `create_memory_object`    | Create a MemoryObject kernel object.                            |
| `4`       | `map_memory_object`       | Map a MemoryObject into an AddressSpace.                        |
| `5`       | `create_channel`          | Create a channel, returning handles to the two ends.            |
| `6`       | `send_message`            | Send a message down a channel.                                  |

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

### Return values
Often, a system call will need to return a status, plus one or more handles. The first handle a system call needs
to return (often the only handle returned) can be returned in the upper bits of the status value:
* Bits `0..32` contain the status:
    - `0` means that the system call succeeded, and the rest of the return value is valid
    - `>0` means that the system call errored. The meaning of the value is system-call specific.
* Bits `32..64` contain the value of the first returned handle, if applicable

A return value of `0xffffffffffffffff` (the maximum value of `u64`) is reserved for when a system call is made with
a number that does not correspond to a system call. This is defined as a normal error code (as opposed to, for
example, terminating the task that tried to make the system call) to provide a mechanism for tasks to detect kernel
support for a system call (so they can use a fallback method on older kernels, for example).
