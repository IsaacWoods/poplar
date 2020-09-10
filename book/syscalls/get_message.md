# `get_message`
Receive a message from a `Channel`, if one is waiting to be received.

A maximum of 4 handles can be transferred by each message. The maximum number of bytes is currently 4096.

### Parameters
`a` - the handle to the `Channel` end that is receiving the message. The handle must have the `RECEIVE` right.
`b` - a pointer to the array of bytes to put the message into
`c` - the size of the bytes buffer
`d` - a pointer to the array of handle entries to transfer. This may be `0x0` if the receiver does not expect to receive any handles.
`e` - the size of the handles buffer (in handles)

### Returns
Bits `0..16` are a status code:
- `0` if the message was received successfully. The rest of the return value is valid.
- `1` if the `Channel` handle is invalid.
- `2` if the `Channel` handle does not point to a `Channel`.
- `3` if there was no message to receive.
- `4` if the address of the bytes buffer is invalid.
- `5` if the bytes buffer is too small to contain the message.
- `6` if the address of the handles buffer is invalid, or if `0x0` was passed and the message does contain handles.
- `7` if the handles buffer is too small to contain the handles transferred with the message.

If the status code is `0` (i.e. a valid message was written into the bytes and handles buffers), the return value
also contains the number of valid entries in both the byte and handle buffers:
- Bits `16..32` contain the length of the valid byte buffer (in bytes). If the passed buffer was larger than this, the
remaining bytes have not been written by the kernel.
- Bits `32..48` contain the length of the valid handles buffer (in handles). If the passed buffer was larger than
this, the remaining bytes have not been written by the kernel.

### Capabilities needed
None.
