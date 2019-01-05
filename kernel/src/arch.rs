/// Trait that is implemented by a type in each architecture module, and passed to `kernel_main`.
/// Provides a common interface to platform-specific operations for the architecture-independent
/// parts of the kernel.
pub trait Architecture: Drop {}
