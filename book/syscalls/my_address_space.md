# `my_address_space`
Get the ID of the AddressSpace kernel object that the calling task is running in. Tasks do not need a
capability to use this system call, as they automatically have access to their own AddressSpaces, and more
priviledged operations are protected by their own capabilities.

### Parameters
None.

### Returns
The kernel object ID of the AddressSpace of the calling task.

### Capabilities needed
None.
