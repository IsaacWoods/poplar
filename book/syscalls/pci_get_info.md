# `pci_get_info`
Get information about the PCI devices on the platform. This is only meant to be used from the userspace PCI bus
driver.

TODO: detail structure of PCI descriptor

### Parameters
`a` - a pointer to the buffer to put the PCI descriptors in
`b` - the size of the buffer (in descriptors)

### Returns
Bits `0..16` contain a status code:
- `0` if the system call succeeded
- `1` if the task does not have the correct capabilities
- `2` if the given buffer can't hold all the descriptors
- `3` if the address to the descriptor buffer is invalid

If the status code is `0` (i.e. the system call succeeded), bits `16..48` contain the number of descriptors written
back.

### Capabilities needed
Tasks need the `PciBusDriver` capability to use this system call.
