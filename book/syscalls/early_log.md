### `early_log`
Used by tasks that are started early in the boot process, before reliable userspace logging support is running.
Output is logged to the same place as kernel logging.

### Parameters
- `a` - the length of the string to log in bytes. Maximum length is 1024 bytes.
- `b` - a usermode pointer to the start of the UTF-8 encoded string.

### Returns
- `0` if the system call succeeded
- `1` if the string was too long
- `2` if the string was not valid UTF-8
- `3` if the task making the syscall doesn't have the `EarlyLogging` capability

### Capabilities needed
The `EarlyLogging` capability is needed to make this system call.
