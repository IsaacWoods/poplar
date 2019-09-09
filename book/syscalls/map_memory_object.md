# `map_memory_object`
Map a MemoryObject into an AddressSpace. This requires the calling task to have access to the MemoryObject,
and to the AddressSpace.

### Parameters
`a` - the kernel object ID of the MemoryObject.
`b` - the kernel object ID of the AddressSpace to map the MemoryObject into.

### Returns
 - `0` if the system call succeeded
 - `1` if the portion of the AddressSpace that would be mapped is already occupied by another MemoryObject
 - `2` if the calling task doesn't have access to the MemoryObject
 - `3` if the calling task doesn't have access to the AddressSpace
 - `4` if the ID for the MemoryObject does not point to a valid MemoryObject, or if the ID does not point to
     any object
 - `5` if the ID for the AddressSpace does not point to a valid AddressSpace, or if the ID does not point to
     any object

### Capabilities needed
None (this may change in the future).
