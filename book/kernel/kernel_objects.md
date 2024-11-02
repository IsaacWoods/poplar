# Kernel Objects
Kernel objects represent resources that are managed by the kernel, that userspace tasks may want to interact with
through system calls.

### Handles
Kernel objects are referenced from a userspace task using *handles*. From userspace, handles are opaque 32-bit
integers, and are associated within the kernel to kernel objects through a per-task mapping.

A handle of value `0` is never associated with a kernel object, and can act as a sentinel value - various system
calls use this value for various meanings.

Each handle is associated with a series of permissions that dictate what the owning userspace task can do with
the corresponding object. Some permissions are relevant to all types of kernel object, while others have meanings
specific to the type of object the handle is associated with.

Permissions (TODO: expand on these):
- Clone (create a new handle to the referenced kernel object)
- Destroy (destroy the handle, destroying the object if no other handle references it)
- Send (send the handle over a `Channel` to another task)

### Address Space
TODO

### Memory Object
TODO

### Task
TODO

### Channel
TODO

### Event
TODO