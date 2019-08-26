# Kernel Object IDs
Pebble uses a slighly unusual system for allocating IDs for kernel objects. Inspired by an idea from
[Catherine West's Rustconf 2018 keynote](https://www.youtube.com/watch?v=aKLntZcp27M), we use **generational
IDs**. This means that kernel object IDs are comprised of two different numbers:

``` rust
pub struct KernelObjectId {
    index: u16,
    generation: u16,
}
```

Note that the meaning of these numbers is only related to how the kernel manages the allocation of IDs, and
can be treated as a single ID by other parts of the system, and by tasks.

The key motivation behind this is called [the ABA problem](https://en.wikipedia.org/wiki/ABA_problem). Because
Pebble makes heavy use of message passing, tasks are expected to cache the IDs of various kernel objects that
they want to interact with often. However, because tasks can be created and destroyed at any point,
conventional IDs could lead to tasks messaging unrelated tasks that have been allocated the ID of a
previously-cached task. As shown by this example, this could even potentially lead to a malicious task
being able to intercept messages meant for something else:

* A logging task is created and allocated ID `17`
* Task A is created and wants to log stuff, so it looks up and caches the logger's ID
* The logger is destroyed, leaving ID `17` free to allocate again
* A malicious task is created, and is allocated ID `17`
* Task A sends a message to ID `17` - which is now not the logger, but this malicious task
* The malicious task receives this message and exfiltrates it!

Of course, this example is slightly convoluted because tasks should not be logging sensitive information in
the first place, but hopefully it's easy to see how this allocation scheme could lead to problems.

Generational IDs solve this problem by introducing a second component to the ID - the generation counter. When
a task is created, it is inserted into the generational data structure and allocated a free index - this
forms the first part of the ID. The generation of the ID is the current generation of that index in the data
structure. When the task is destroyed, it is removed from the data structure and its index's generation is
incremented. If you try to get the element at a given index, but the generations don't match, the data
structure acts if there's nothing at that index, because the thing you think is there no longer will be!

With this new ID system, let's see what happens in the same situation with the logger:

* A logger task is created and allocated ID `(17, 0)`
* Task A is created and wants to log stuff, so it looks up and caches the logger's ID
* The logger is destroyed, and the generation of ID `17` is increased from `0` to `1`
* A malicious task is created, and is allocated ID `(17, 1)`
* Task A sends a message to ID `(17, 0)` - which is now not the logger and no longer exists
* Task A receives a message saying the task at `(17, 0)` no longer exists
* The malicious task fails to intercept Process A's logging

And with that, our very convoluted malicious task has been defeated!
