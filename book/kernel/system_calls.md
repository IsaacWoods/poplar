# System calls
Userspace code can interact with the kernel through system calls. Poplar's system call interface is based around
'kernel objects', and so many of the system calls are to create, destroy, or modify the state of various types of
kernel object. Because of Poplar's microkernel design, many traditional system calls (e.g. `open`) are not present,
their functionality instead being provided by userspace.

Each system call has a unique number that is used to identify it. A system call can then take up to five
parameters, each a maximum in size of the system's register width. It can return a single value, also the size of
a register.

### Overview of system calls

| Number    | System call               | Description                                                           |
|-----------|---------------------------|-----------------------------------------------------------------------|
| `0`       | `yield`                   | Yield to the kernel.                                                  |
| `1`       | `early_log`               | Log a message. Designed to be used from early processes.              |
| `3`       | `create_memory_object`    | Create a MemoryObject kernel object.                                  |
| `4`       | `map_memory_object`       | Map a MemoryObject into an AddressSpace.                              |
| `5`       | `create_channel`          | Create a channel, returning handles to the two ends.                  |
| `6`       | `send_message`            | Send a message down a channel.                                        |
| `7`       | `get_message`             | Receive the next message, if there is one.                            |
| `8`       | `wait_for_message`        | Yield to the kernel until a message arrives on the given              |
| `12`      | `wait_for_event`          | Yield to the kernel until an event is signalled                       |
| `13`      | `poll_interest`           | Poll a kernel object to see if changes need to be processed.          |
| `14`      | `create_address_space`    | Create an AddressSpace kernel object.                                 |
| `15`      | `spawn_task`              | Create a Task kernel object and start scheduling it.                  |

Deprecated:
| Number    | System call               | Description                                                           |
|-----------|---------------------------|-----------------------------------------------------------------------|
| `2`       | `get_framebuffer`         | Get the framebuffer that the kernel has created, if it has.           |
| `9`       | `register_service`        | Register yourself as a service.                                       |
| `10`      | `subscribe_to_service`    | Create a channel to a particular service provider.                    |
| `11`      | `pci_get_info`            | Get information about the PCI devices on the platform.                |

### Making a system call on x86_64
To make a system call on x86_64, populate these registers:

| `rdi`     | `rsi` | `rdx` | `r10` | `r8`  | `r9`  |
|-----------|-------|-------|-------|-------|-------|
| number    | `a`   | `b`   | `c`   | `d`   | `e`   |

The only way in which these registers deviate from the x86_64 Sys-V ABI is that `c` is passed in `r10` instead of
`rcx`, because `rcx` is used by the `syscall` instruction.  You can then make the system call by executing
`syscall`. Before the kernel returns to userspace, it will put the result of the system call (if there is one) in
`rax`. If a system call takes less than five parameters, the unused parameter registers will be preserved across
the system call.

### Making a system call on RISC-V
TODO

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

### Syscall: `yield`
Yield to the kernel. Generally called when a userspace task has no more useful work to perform.

- Parameters:
    - None
- Returns:
    - Always `0`

### Syscall: `early_log`
Output a line to the kernel log. This is generally used by tasks early in the boot process, before reliable userspace logging
is running, but could also be used by small userspaces for diagnostic logging. The output to be logged must be provided as a
formatted string encoded as UTF-8.

- Parameters:
    - `a`: the length of the string to log, in bytes. Max of `4096` bytes.
    - `b`: the address of the string to log.
- Returns:
    - `0`: success
    - `1`: the length supplied is too large
    - `2`: the supplied string is not valid UTF-8

### Syscall: `create_memory_object`
Create a `MemoryObject` kernel object. Userspace can only create "blank" memory objects, backed by free, conventional physical memory.

- Parameters:
    - `a`: the length of the memory object, in bytes
    - `b`: flags:
        - Bit `0`: set if the memory should be writable
        - Bit `1`: set if the memory should be executable
    - `c`: an address to which the kernel will write the physical address to which the memory object was allocated. Not written if null.
- Returns:
    - `0`: success
    - `1`: the given set of flags is invalid
    - `2`: a memory area of the requested size could not be allocated
    - `3`: the address in `c` is not null, but is not valid

### Syscall: `map_memory_object`
Map a `MemoryObject` into an `AddressSpace`.

- Parameters:
    - `a`: the handle of the `MemoryObject`
    - `b`: the handle of the `Addressspace`. A zero handle indicates that the memory object should be mapped into the task's address space.
    - `c`: the virtual address to map the memory object at. Null indicates that the kernel should attempt to find a region in the address space large enough to hold the memory object and map it there.
    - `d`: a pointer to which the virtual address the memory object has been mapped to is written, if `c` is null. If `d` is null, this address is not written.
- Returns:
    - `0`: success
    - `1`: the handle to the `MemoryObject` is invalid or does not point to a `MemoryObject`
    - `2`: the handle to the `AddressSpace` is invalid or does not point to a `AddressSpace`
    - `3`: the region of the address space that would be mapped is alreay occupied
    - `4`: the supplied pointer in `d` is invalid

### Syscall: `create_channel`
Create a new channel, returning handles to two `Channel` objects, each representing an end of the channel. Generally, one of these handles
is sent to another task to facilitate IPC.

- Parameters:
    - `a`: the address to write the second handle to (only one can be returned in the status)
- Returns:
    - Status in bits `0..32`:
        - `0`: success
        - `1`: the virtual address to write the second handle to is invalid
    - Handle to first end in bits `32..64`

TODO: we could pack both handles into the return value by using a sentinel `0` handle to mark that the other handle is actually an error?

### Syscall: `send_message`
Send a message, consisting of a number of bytes and optionally a number of handles, down a `Channel`.
All the handles are removed from the sending `Task` and added to the receiving `Task`.

A maximum of 4 handles can be transferred by each message. The maximum number of bytes is currently 4096.

- Parameters:
    - `a`: the handle to the `Channel` from which the message is to be sent
    - `b`: a pointer to the array of bytes to send
    - `c`: the length of the message, in bytes
    - `d`: a pointer to the array of handle entries to transfer. If the message does not transfer any handles, this should be `0x0`
    - `e`: the number of handles to transfer
- Returns:
    - `0` if the system call succeeded and the message was sent
    - `1` if the `Channel` handle is invalid
    - `2` if the `Channel` handle does not point to a `Channel`
    - `3` if the `Channel` handle does not have the correct rights to send messages
    - `4` if one or more of the handles to transfer is invalid
    - `5` if any of the handles to transfer do not have the correct rights
    - `6` if the pointer to the message bytes was not valid
    - `7` if the message's byte array is too large
    - `8` if the pointer to the handles array was not valid
    - `9` if the handles array is too large
    - `10` if the other end of the `Channel` has been disconnected

### Syscall: `get_message`
Receive a message from a `Channel`, if one is waiting to be received.

A maximum of 4 handles can be transferred by each message. The maximum number of bytes is currently 4096.

- Parameters:
    - `a`: the handle to the `Channel` end that is receiving the message.
    - `b`: a pointer to the array of bytes to write the message to
    - `c`: the maximum number of bytes the kernel should attempt to write to the buffer at `b`
    - `d`: a pointer to the array of handle entries to transfer. This can be `0x0` if the receiver does not expect to receive any handles.
    - `e`: the maximum number of handles the kernel should attempt to write to the array at `d`.
- Returns:
    - Status in bits `0..16`:
        - `0` if the message was received successfully. The rest of the return value is valid.
        - `1` if the `Channel` handle is invalid.
        - `2` if the `Channel` handle does not point to a `Channel`.
        - `3` if there was no message to receive.
        - `4` if the address of the bytes buffer is invalid.
        - `5` if the bytes buffer is too small to contain the message.
        - `6` if the address of the handles buffer is invalid, or if `0x0` was passed and the message does contain handles.
        - `7` if the handles buffer is too small to contain the handles transferred with the message.
    - The length of the message in bits `16..32`
        - This is only valid for statuses of `0`
    - The number of handles tranferred in bits `32..48`
        - This is only valid if statuses of `0`

### Syscall: `wait_for_message`
TODO

### Syscall: `wait_for_event`
TODO

### Syscall: `poll_interest`
TODO

### Syscall: `create_address_space`
TODO

### Syscall: `spawn_task`
TODO