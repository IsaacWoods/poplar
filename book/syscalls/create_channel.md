# `create_channel`
Create a `Channel` kernel object. Channels are slightly odd kernel objects in that they must be referred to in
userspace by two handles, one for each "end" of the channel. This system call therefore returns two handles, one of
which is usually transferred to another task.

### Parameters
- `a` - the virtual address to write the second handle into (only one can be returned in the status)

### Returns
Uses the standard representation to return a `Result<Handle, CreateChannelError>` method. Error status
codes are:
- `1` if the passed virtual address is not valid

TODO: if we ditch the ability to return an error (i.e. by making this infallible, or by saying that a null handle
denotes an error but not which one), we could return both handles in the status.

### Capabilities needed
None.
