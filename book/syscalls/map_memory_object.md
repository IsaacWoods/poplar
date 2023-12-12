# `map_memory_object`
Map a MemoryObject into an AddressSpace.

### Parameters
- `a` - a handle to the MemoryObject.
- `b` - a handle to the AddressSpace. The zero handle indicates to map the memory object into the task's AddressSpace.
- `c` - the virtual address to map the MemoryObject at, if it should be mapped at a specific address. If `null`,
        the kernel will attempt to find a suitable address to map it at, and write that address to the pointer
        supplied in `d`.
- `d` - the pointer at which the virtual address the object is mapped at will be written to, if `c` is `null`. If
        an address is supplied in `c`, this pointer does not need to be valid, and will not be accessed. If this
        pointer is `null`, the address will not be written, even if the kernel allocated memory for the object.

### Returns
- `0` if the system call succeeded
- `1` if either of the passed handles are invalid
- `2` if the portion of the AddressSpace that would be mapped is already occupied by another MemoryObject
- `3` if the supplied MemoryObject handle does not point to a MemoryHandle
- `4` if the supplied AddressSpace handle does not point to an AddressSpace
- `5` if the supplied pointer in `d` is invalid, and `c` is `null`

### Capabilities needed
None (this may change in the future).
