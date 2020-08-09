# Efiloader
`efiloader` is the bootloader for Pebble on x86_64. It utilities UEFI boot-services to load the kernel and any
extra images needed into memory, allocate memory for the heap, configure a basic framebuffer, and enter the kernel.

### Description of booting process
A rough order of the steps that `efiloader` performs is:
- Parses a set of load options passed to the loader, allowing the user to instruct it on how to load the kernel
- Finds the physical address of the RSDP, so the kernel can find the ACPI tables
- Creates a basic framebuffer using the UEFI GOP (Graphics Output Protocol), if requested
- Allocate and map a heap for the kernel to use
- Load any additional images needed from the filesystem
- Constructs some "boot info", including a map of physical memory, telling the kernel about the hardware
- Jumps into the kernel

### Load options
A series of load options may be supplied to `efiloader` to tell it how Pebble should be booted. These options
consist of a string of space separated key-value pairs, of the form `a.dot.separated.key=value`. Supported keys,
plus descriptions of their values, are:

| Key               | Example value             | Description                                                                                                           |
|-------------------|---------------------------|-----------------------------------------------------------------------------------------------------------------------|
| `kernel`          | `kernel.elf`              | The path within the ESP that the kernel should be loaded from.                                                        |
| `fb.none`         | No value                  | Specify that a GOP framebuffer should not be created.                                                                 |
| `fb.width`        | `1920`                    | Specify that a GOP framebuffer should be created, and its width.                                                      |
| `fb.height`       | `1080`                    | Specify that a GOP framebuffer should be created, and its height.                                                     |
| `image.{name}`    | `my_task.elf`             | Specifies a path that an additional image should be loaded from. The key is the name that is passed in the boot info. |

If no load options are supplied, a kernel will be loaded from `\kernel.elf`, no additional images will be loaded,
and a GOP framebuffer with a width of `800` and a height of `600` will be created. 
