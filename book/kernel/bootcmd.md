# Bootcmd
The `bootcmd` is a file loaded by the bootloader that contains instructions for booting into an instance of Pebble.
It is located at the root of the boot medium (e.g. in the FAT partition if using the UEFI bootloader for x86_64),
and consists of a series of line-separated commands that load / configure parts of the OS.

### Commands
* `kernel {path}` - load the kernel image at the specified path into memory. This command must be present for the
  bootloader to successfully boot, and must only appear once in the `bootcmd`.
* `image {path} {name}` - load an initial task image into memory and pass information about it to the kernel. This
  command is used to load initial tasks for the kernel to run before it has functioning filesystem drivers. It
  can also be used to load all tasks needed in minimal systems. It can be present zero or more times in the
  `bootcmd`. The same image should not be loaded more than once, however.
* `video_mode {desired width} {desired height}` - attempts to set up a video mode for initial graphics support (if
  a better driver isn't available / loaded). The exact behaviour of this command will depend on the platform
  being booted upon, but it will in general try to switch to an appropriate mode, pass information about that
  mode to the kernel, which will pass it on to the task with the `CAP_MAP_FRAMEBUFFER` capability (refer to
  the 'Userspace/Capabilities' section for more information)
