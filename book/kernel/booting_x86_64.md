# Booting Pebble on x86_64
On x86_64, Pebble is booted by a UEFI application that can be found [here](https://github.com/pebble-os/pebble/tree/master/bootloader). We don't support booting from BIOS, mainly because the vast majority
of current platforms now support UEFI, and I didn't want the maintenance burden of two bootloaders for one platform. Pebble's kernel and bootloaders are more coupled than on other platforms, and the
bootloader does a lot more of the platform-specific initialization than generic bootloaders like GRUB2. On x86_64, doing this initialization in the bootloader allows us to use the Boot Services provided by
the UEFI, which simplifies early bring-up considerably.

The bootloader is responsible for setting up a sensible environment for the kernel to run in, as well as doing initialization that makes use of the Boot Services that are not available after we've loaded the
kernel. This is much easier than it would be if we supported the BIOS, as UEFI already defines a fairly sane environment. The bootloader:
* Finds and switches to a suitable graphics mode
* Loads the kernel's image from the boot partition, loads its sections, and creates a set of page tables for it
* Loads the payload's image from the boot partition, loads its sections, and creates a set of page tables for it
* Allocates backing memory for the kernel heap
* Sets up paging and creates page tables with the correct kernel-space mappings
* Jumps into the kernel

### Loading the kernel
The kernel is an ELF file on the boot partition. Only its allocatable sections are mapped into memory. The kernel is expected to have a couple of special symbols defined, which are used by the bootloader:

* `_guard_page` - the start address of the guard page. This is purposefully unmapped by the bootloader, so that stack overflows cause page-faults
* `_stack_bottom` and `_stack_top` - define the bottom and top of the stack, respectively. These are defined as part of the `.bss` section, so the bootloader doesn't have to manually allocate a stack. The 
address is provided so the bootloader knows what to set `rsp` to before it jumps into the kernel

### Loading the kernel payload
While we have the luxury of the UEFI's filesystem drivers being available, we also load another ELF image called the 'kernel payload'. This is the first process launched by the kernel, and is usually
responsible for starting a set of other processes from an embedded initial filesystem. Alternatively, a simple application can be provided as a payload, for example in embedded contexts.

While it seems like a strange choice, we load this in the bootloader so that the kernel doesn't have to have any logic to do so. Pebble The Microkernel has no concept of files, of ramdisks, or of what an ELF
file looks like inside - it simply accepts sets of page tables and created processes out of them. This makes the kernel much simpler and less bug-prone, at the expense of making the bootloader slightly more
complex.
