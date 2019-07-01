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

### List of system calls

| Number    | System call           | a         | b         | c         | d         | e         | Return value          | Description                                                                   |
|-----------|-----------------------|-----------|-----------|-----------|-----------|-----------|-----------------------|-------------------------------------------------------------------------------|
| `0`       | `yield`               | -         | -         | -         | -         | -         | -                     | Yield to the kernel.                                                          |

### Making a system call on x86_64
To make a system call on x86_64, populate these registers:

| `rax`                 | `rdi` | `rsi` | `rdx` | `r8`  | `r9`  |
|-----------------------|-------|-------|-------|-------|-------|
| System call number    | `a`   | `b`   | `c`   | `d`   | `e`   |

You can then make the system call by executing `syscall`. Before the kernel returns to userspace, it will put the result of the system call (if there is one) in `rax`.
If a system call takes less than five parameters, the unused parameter registers will be preserved across the system call.
