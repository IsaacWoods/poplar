/// This structure is placed in memory by the bootloader and a reference to it passed to the
/// kernel. It allows the kernel to access information discovered by the bootloader, such as the
/// graphics mode it switched to.
pub struct BootInfo {}
