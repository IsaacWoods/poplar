# Process IDs

Pebble uses a slighly different scheme for allocating process IDs to many other kernels. Inspired by an idea from [Catherine West's Rustconf 2018 keynote](https://www.youtube.com/watch?v=aKLntZcp27M), Pebble
uses **generational IDs** for its PIDs. This means PIDs are comprised of two different numbers:

``` rust
pub struct ProcessId {
    index: u16,
    generation: u16,
}
```

Note that the meaning of these numbers is only related to how the kernel manages the allocation of IDs, and can be treated as a single ID by other parts of the system, and by programs.

The key motivation behind this is called [the ABA problem](https://en.wikipedia.org/wiki/ABA_problem). Because Pebble makes heavy use of message passing, programs are expected to cache the PIDs of processes
they want to message often, such as the VFS process or a logging service. However, because processes can be created and destroyed at any point, conventional PIDs could lead to processes messaging unrelated
programs that have been allocated the PID of a previously-cached process. As shown by this example, this could even potentially lead to a malicious process being able to intercept messages meant for something
else:

* A logging process is created and allocated PID `17`
* Process A is created and wants to log stuff, so it looks up and caches the logger's PID
* The logger is destroyed, leaving PID `17` free to allocate again
* A malicious process is created, and is allocated PID `17`
* Process A sends a message to PID `17` - which is now not the logger, but this malicious process
* The malicious process receives this message and exfiltrates it!

Of course, this example is slightly convoluted because programs should not be logging sensitive information in the first place, but hopefully it's easy to see how this allocation scheme could lead to problems.

Generational IDs solve this problem by introducing a second component to the ID - the generation counter. When a process is created, it is inserted into the generational data structure and allocated a free
index - this forms the first part of the PID. The generation of the PID is the current generation of that index in the data structure. When the process is destroyed, it is removed from the data structure and
its index's generation is incremented. If you try to get the element at a given index, but the generations don't match, the data structure acts if there's nothing at that index, because the thing you think
is there no longer will be!

With this new ID system, let's see what happens in the same situation with the logger:

* A logger process is created and allocated PID `(17, 0)`
* Process A is created and wants to log stuff, so it looks up and caches the logger's PID
* The logger is destroyed, and the generation of PID `17` is increased from `0` to `1`
* A malicious process is created, and is allocated PID `(17, 1)`
* Process A sends a message to PID `(17, 0)` - which is now not the logger and no longer exists
* Process A receives a message saying the process at `(17, 0)` no longer exists
* The malicious process fails to intercept Process A's logging

And with that, our very convoluted malicious process has been defeated!
