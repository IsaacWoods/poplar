# `map_memory_object`
Map a MemoryObject into an AddressSpace.

### Parameters
`a` - a handle to the MemoryObject.
`b` - a handle to the AddressSpace. The zero handle indicates to map the memory object into the task's AddressSpace.
`c` - a pointer to which the kernel will write the virtual address at which the MemoryObject was mapped. Ignored if null.

### Returns
- `0` if the system call succeeded
- `1` if either of the passed handles are invalid
- `2` if the portion of the AddressSpace that would be mapped is already occupied by another MemoryObject
- `3` if the supplied MemoryObject handle does not point to a MemoryHandle
- `4` if the supplied AddressSpace handle does not point to an AddressSpace
- `5` if the pointer to write the virtual address back to is invalid

### Capabilities needed
None (this may change in the future).
