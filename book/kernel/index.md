### Arch modules
Each architecture has its own module that deals with platform-specific initialization and platform management.
When building, a single architecture module is selected using an architecture feature. Through conditional
compilation, only the selected architecture module is compiled and included in the final kernel executable.

Every architecture module is expected to provide certain items for the benefit of the platform-independent
parts of the kernel:
* `Arch` - A type that implements the `Architecture` trait.
* `common_per_cpu_data` - A function to access the common per-cpu data.
* `common_per_cpu_data_mut` - A function to mutably access the common per-cpu data.
* `context_switch` - A function to perform a context switch between the currently running task and another task.
