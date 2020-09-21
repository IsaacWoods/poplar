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
| First byte    | Next byte(s)  | Data                  | Arch specific?    | Description                                                           |
|---------------|---------------|-----------------------|-------------------|-----------------------------------------------------------------------|
| `0x00`        | -             | -                     | -                 | No meaning - used to pad descriptor to required length (see above)    |
| `0x01`        |               |                       | No                | `GetFramebuffer`                                                      |
| `0x02`        |               |                       | No                | `EarlyLogging`                                                        |
| `0x03`        |               |                       | No                | `ServiceProvider`                                                     |
| `0x04`        |               |                       | No                | `ServiceUser`                                                         |
| `0x05`        | -             | -                     | No                | `PciBusDriver`                                                        |
