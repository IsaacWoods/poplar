# `create_memory_object`
Create a MemoryObject kernel object. Userspace can only create "blank" MemoryObjects (that are allocated to free,
conventional physical memory). MemoryObjects that point to special objects (e.g. framebuffer data, PCI
configuration spaces) must be created by the kernel.

### Parameters
`a` - the virtual address to map the MemoryObject at
`b` - the size of the MemoryObject's memory area (in bytes)
`c` - flags:
  - Bit `0`: set if the memory should be writable
  - Bit `1`: set if the memory should be executable

### Returns
Uses the standard representation to return a `Result<Handle, MemoryObjectError>` method. Error status
codes are:
- `1` if the given virtual address is invalid
- `2` if the given set of flags are invalid
- `3` if memory of the requested size could not be allocated

### Capabilities needed
None.
