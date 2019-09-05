# System calls
Userspace code can interact with the kernel through system calls. Unlike traditional monolithic kernels, Pebble's system call interface is designed to be very minimal; many of the operations traditionally
supported by system calls (such as filesystem operations) are not provided by the kernel in Pebble, and so are instead accessed through passing messages to other userspace processes.

Pebble's original design had userspace processes communicate with the kernel by passing messages to it, like it would communicate with another userspace process. Having a system call interface has a few
advantages over this design:
* System calls have much less overhead
* Programs that otherwise wouldn't need to pass messages don't need the extra machinery to talk to the kernel
* The kernel no longer has to deserialize messages. While it still contains some message-passing infrastructure, it only needs to pass the headers and `memcpy` stuff to the right place. This hugely reduces the
attack surface of the kernel.

Each system call has a unique number that is used to identify it. A system call can then take up to five parameters, each a maximum in size of the system's register width. It can return a single value, also
the size of a register.

### Overview of system calls

| Number    | System call               | a                 | b                 | c                 | d                 | e                 | Return value          | Description                                               |
|-----------|---------------------------|-------------------|-------------------|-------------------|-------------------|-------------------|-----------------------|-----------------------------------------------------------|
| `0`       | `yield`                   | -                 | -                 | -                 | -                 | -                 | -                     | Yield to the kernel.                                      |
| `1`       | `early_log`               | length of string  | ptr to string     | -                 | -                 | -                 | success / error       | Log a message. Designed to be used from early processes.  |
| `2`       | `request_system_object`   | object id         | {depends on id}   | {depends on id}   | {depends on id}   | {depends on id}   | id of object + status | Request the id of a system kernel object.                 |
| `3`       | `my_address_space`        | -                 | -                 | -                 | -                 | -                 | AddressSpace id       | Get the id of the calling task's AddressSpace.            |
| `4`       | `map_memory_object`       | MemoryObject id   | AddressSpace id   | -                 | -                 | -                 | success / error       | Map a MemoryObject into an AddressSpace.                  |

### Making a system call on x86_64
To make a system call on x86_64, populate these registers:

| `rdi`                 | `rsi` | `rdx` | `r10` | `r8`  | `r9`  |
|-----------------------|-------|-------|-------|-------|-------|
| System call number    | `a`   | `b`   | `c`   | `d`   | `e`   |

The only way in which these registers deviate from the x86_64 Sys-V ABI is that `c` is passed in `r10` instead
of `rcx`. This is because `rcx` is used by the `syscall` instruction, and so is not free.
You can then make the system call by executing `syscall`. Before the kernel returns to userspace, it will put the result of the system call (if there is one) in `rax`.
If a system call takes less than five parameters, the unused parameter registers will be preserved across the system call.

### `yield`
Used by a task that can't do any work at the moment, allowing the kernel to schedule other tasks.

### `early_log`
Used by tasks that are started early in the boot process, before reliable userspace logging support is running. Output is
logged to the same place as kernel logging.

This system call should not be used from standard usermode tasks, and so requires the `EarlyLogging` capability to use.
The first parameter (`a`) is the length of the string in bytes, and the second (`b`) is a UTF-8 encoded string that is not
null-terminated. The maximum length of the string is 1024 chars.

Returns:
 - `0` if the system call succeeded
 - `1` if the string was too long
 - `2` if the string was not valid UTF-8
 - `3` if the task making the syscall doesn't have the `EarlyLogging` capability

### `request_system_object`
Used by tasks to request access to a "system" kernel object - usually one created by the kernel to provide
some resource, such as the framebuffer, to userspace. Each object has a hardcoded id used to request it, and
requires the requesting task to have a particular capability - if the task is permitted access to the object,
the kernel returns the kernel object id of the object, and takes any steps needed for the requesting task to
be able to access the object. Normal user tasks probably don't have any need for this system call - it is more
aimed at device drivers and system management tasks.

If this system call is successful, access is granted to the system object from the calling task. This means it
can use the returned id in other system calls.

The first parameter, `a`, is always the id (not to be confused with the actual kernel object id, which is not
hardcoded and therefore can change between boots) of the system object. The allowed values are:

| `a`   | Object being requested                | Type              | `b`           | `c`           | `d`           | `e`           | Required capability       |
|-------|---------------------------------------|-------------------|---------------|---------------|---------------|---------------|---------------------------|
| `0`   | The backup framebuffer                | `MemoryObject`    | -             | -             | -             | -             | `AccessBackupFramebuffer` |

TODO: id for accessing Pci config space where extra params are bus, device, function (+segment or whatever)
numbers.

Returns:
 * Bits `0..16` contain the index of the requested object's ID, if the system call succeeded
 * Bits `16..32` contain the generation of the requested object's ID, if the system call succeeded
 * Bits `32..63` contain the status of the system call:
    - `0` means the system call succeeded and bits `0..32` hold a valid kernel object id
    - `1` means that the requested object is a valid system object, but does not exist
    - `2` means that the id does not correspond to a valid system object
    - `3` means that the requested object id is valid, but the task does not have the correct capabilities to
      access it

### `my_address_space`
Get the ID of the AddressSpace kernel object that the calling task is running in. Tasks do not need a
capability to use this system call, as they automatically have access to their own AddressSpaces, and more
priviledged operations are protected by their own capabilities.

### `map_memory_object`
Map a MemoryObject into an AddressSpace. This requires the calling task to have access to the MemoryObject,
and to the AddressSpace.

The first parameter, `a`, is the kernel object ID of the MemoryObject. The second parameter, `b`, is the
kernel object ID of the AddressSpace to map the MemoryObject into.

Returns:
 - `0` if the system call succeeded
 - `1` if the portion of the AddressSpace that would be mapped is already occupied by another MemoryObject
 - `2` if the calling task doesn't have access to the MemoryObject
 - `3` if the calling task doesn't have access to the AddressSpace
 - `4` if the ID for the MemoryObject does not point to a valid MemoryObject, or if the ID does not point to
     any object
 - `5` if the ID for the AddressSpace does not point to a valid AddressSpace, or if the ID does not point to
     any object
