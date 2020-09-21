# `get_framebuffer`
On many architectures, the bootloader or kernel can create a naive framebuffer using a platform-specific method.
This framebuffer can be used to render from userspace, if a better hardware driver is not available on the
platform.

### Parameters
- `a` should contain a mapped, writable, user-space address, to which information about the framebuffer will
  be written.

### Returns
This system call returns three things:
- A status code
- A handle to a `MemoryObject` containing the framebuffer, if successful
- Information about the framebuffer, if successful, written into the address in `a`

The status codes used are:
- `0` means that the system call was successful
- `1` means that the calling task does not have the correct capability
- `2` means that `a` does not contain a valid address for the kernel to write to
- `3` means that the kernel did not create the framebuffer

The information written back to the address in `a` has the following structure:
``` rust
#[repr(C)]
struct FramebufferInfo {
    width: u16,
    height: u16,
    stride: u16,
    /// 0 = RGB32
    /// 1 = BGR32
    pixel_format: u8,
}
```

### Capabilities needed
Tasks need the `GetKernelFramebuffer` capability to use this system call.
