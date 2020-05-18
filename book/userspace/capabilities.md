# Capabilities
Capabilities describe what a task is allowed to do, and are encoded in its image. This allows users to audit the
permissions of the tasks they run at a much higher granularity than user-based permissions, and also allow us to
move parts of the kernel into discrete userspace tasks by creating specialised capabilities to allow access to
sensitive resources (such as the raw framebuffer) to only select tasks.

### Encoding capabilities in the ELF image
Capabilities are encoded in an entry of a `PT_NOTE` segment of the ELF image of a task. This entry will have an
owner (sometimes referred to in documentation as the 'name') of `PEBBLE` and a type of `0`. The descriptor will be
an encoding of the capabilities as described by the 'Format' section. The descriptor must be padded such that the
next descriptor is 4-byte aligned, and so a value of `0x00` is reserved to be used as padding.

Initial images (tasks loaded by the bootloader before filesystem drivers are working) are limited to a capabilities
encoding of 32 bytes (given the variable-length encoding, this does not equate to a fixed maximum number of
capabilities).

### Format
The capabilities format is variable-length - simple capabilities can be encoded as a single byte, while more
complex / specific ones may need multiple bytes of prefix, and can also encode fixed-length data.

### Overview of capabilities
This is an overview of all the capabilities the kernel supports:
| First byte    | Next byte(s)  | Data                  | Arch specific?    | Description                                                           | Status        |
|---------------|---------------|-----------------------|-------------------|-----------------------------------------------------------------------|---------------|
| `0x00`        | -             | -                     | -                 | No meaning - used to pad descriptor to required length (see above)    | -             |
| `0x01`        |               |                       | No                | `CreateAddressSpace`                                                  | Planned       |
| `0x02`        |               |                       | No                | `CreateMemoryObject`                                                  | Planned       |
| `0x03`        |               |                       | No                | `CreateTask`                                                          | Planned       |
| `0x04`-`0x1f` |               |                       |                   | Reserved for future kernel objects                                    |               |
| `0x20`        | `0x00`        | `u16` port number     | Yes - x86_64      | `X86_64AccessIoPort`                                                  | Planned       |
| `0x21`-`0x2f` |               |                       |                   | Reserved for future architectures                                     |               |
| `0x30`        |               |                       | No                | `GetFramebuffer`                                                      | Implemented   |
| `0x31`        |               |                       | No                | `EarlyLogging`                                                        | Implemented   |
| `0x32`        |               |                       | No                | `ServiceProvider`                                                     | Implemented   |
| `0x33`        |               |                       | No                | `ServiceUser`                                                         | Implemented   |
