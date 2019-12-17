# Kernel Objects
Kernel Objects are how Pebble represents resources that can be interacted with from userspace. They are all
allocated an ID that can be passed into userspace

### Using generational IDs for kernel objects
Pebble, inspired by [Catherine West's Rustconf 2018 keynote](https://www.youtube.com/watch?v=aKLntZcp27M), uses
generational IDs to refer to kernel objects. This means that IDs are comprised of two numbers:
``` rust
pub struct KernelObjectId {
    index: u16,
    generation: u16,
}
```

The meaning of these numbers is internal to how the kernel manages the allocation of IDs, and can be ignored by the
rest of the system.

The key motivation behind this is called the [the ABA problem](https://en.wikipedia.org/wiki/ABA_problem). Because
kernel objects can be created and destroyed at any point, an ID that you've cached can stop pointing to the object
you think it points to - lots of systems solve this problem by never reusing IDs (once an object is destroyed, its
ID is not reallocated). However, this often requires complex data structures to make efficient use of memory, and
so Pebble uses generational IDs instead (where the index remains a simple index into the backing memory).
