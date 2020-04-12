# Kernel Objects
Kernel Objects are how Pebble represents resources that can be interacted with from userspace. They are all
allocated a unique ID.

### Handles
Handles are used to refer to kernel objects from userspace, and are allocated to a single Task.
A handle of value `0` acts as a sentinel value that can be used for special meanings. From userspace, handles
must be treated as opaque, 32-bit integers.
