# `request_system_object`
Used by tasks to request access to a "system" kernel object - usually one created by the kernel to provide
some resource, such as the framebuffer, to userspace. Each object has a hardcoded id used to request it, and
requires the requesting task to have a particular capability - if the task is permitted access to the object,
the kernel returns the kernel object id of the object, and takes any steps needed for the requesting task to
be able to access the object. Normal user tasks probably don't have any need for this system call - it is more
aimed at device drivers and system management tasks.

If this system call is successful, access is granted to the system object from the calling task. This means it
can use the returned id in other system calls.

### Parameters
The first parameter, `a`, is always the id (not to be confused with the actual kernel object id, which is not
hardcoded and therefore can change between boots) of the system object. The meaning of the other parameters
depend on the object requested. The allowed values are:

| `a`   | Object being requested                | Type              | `b`           | `c`           | `d`           | `e`           |
|-------|---------------------------------------|-------------------|---------------|---------------|---------------|---------------|
| `0`   | The backup framebuffer                | `MemoryObject`    | ptr to info   | -             | -             | -             |

TODO: id for accessing Pci config space where extra params are bus, device, function (+segment or whatever)
numbers.

### Returns
 * Bits `0..16` contain the index of the requested object's ID, if the system call succeeded
 * Bits `16..32` contain the generation of the requested object's ID, if the system call succeeded
 * Bits `32..63` contain the status of the system call:
    - `0` means the system call succeeded and bits `0..32` hold a valid kernel object id
    - `1` means that the requested object is a valid system object, but does not exist
    - `2` means that the id does not correspond to a valid system object
    - `3` means that the requested object id is valid, but the task does not have the correct capabilities to
      access it

### Capabilities needed
| id    | Capability needed             |
|-------|-------------------------------|
| `0`   | `AccessBackupFramebuffer`     |

### System object: backup framebuffer
An ID of `0` corresponds to the backup framebuffer system object - a framebuffer created by the bootloader or
kernel that can be used if there is not a more specialized graphics driver available (e.g. on x86_64, this uses the
UEFI Graphics Output Protocol to create a basic linear framebuffer). The object is a `MemoryObject` that is meant
to be mapped into the userspace driver and directly written to as video memory.

A userspace address should be passed as `b`, which is used to pass information about the framebuffer back to
userspace from the kernel. The memory must be user-accessible and writable. The format of the written structure is:
``` rust
#[repr(C)]
struct FramebufferInfo {
    /// The address of the start of the framebuffer
    address: usize,

    width: u16,
    height: u16,
    stride: u16,
    /// 0 = RGB32
    /// 1 = BGR32
    pixel_format: u8,
}
```
