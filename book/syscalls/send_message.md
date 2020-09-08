# `send_message`
Send a message, consisting of a number of bytes and optionally a number of handles, down a `Channel`.
All the handles are removed from the sending `Task` and added to the receiving `Task`.

A maximum of 4 handles can be transferred by each message. The maximum number of bytes is currently 4096.

### Parameters
`a` - the handle to the `Channel` end that is sending the message. The handle must have the `SEND` right.
`b` - a pointer to the array of bytes to send
`c` - the number of bytes to send
`d` - a pointer to the array of handle entries to transfer. All handles must have the `TRANSFER` right. This may be `0x0` if the message does not transfer any handles.
`e` - the number of handles to send

### Returns
A status code:
- `0` if the system call succeeded and the message was send
- `1` if the `Channel` handle is invalid
- `1` if the `Channel` handle does not have the correct rights to send messages
- `3` if one or more of the handles to transfer is invalid
- `4` if any of the handles to transfer do not have the correct rights
- `5` if the pointer to the message bytes was not valid
- `6` if the message's byte array is too large
- `7` if the pointer to the handles array was not valid
- `8` if the handles array is too large

### Capabilities needed
None.
