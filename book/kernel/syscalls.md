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

| Number    | System call           | a                 | b                 | c                 | d                 | e                 | Return value          | Description                                               |
|-----------|-----------------------|-------------------|-------------------|-------------------|-------------------|-------------------|-----------------------|-----------------------------------------------------------|
| `0`       | `yield`               | -                 | -                 | -                 | -                 | -                 | -                     | Yield to the kernel.                                      |
| `1`       | `early_log`           | length            | ptr to string     | -                 | -                 | -                 | success/error         | Log a message. Designed to be used from early processes.  |

### Making a system call on x86_64
To make a system call on x86_64, populate these registers:

| `rax`                 | `rdi` | `rsi` | `rdx` | `r8`  | `r9`  |
|-----------------------|-------|-------|-------|-------|-------|
| System call number    | `a`   | `b`   | `c`   | `d`   | `e`   |

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
