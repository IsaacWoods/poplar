# The Pebble Kernel
This is the Pebble Kernel. It is arranged in a heirarchy of crates, centered around the `kernel` crate:
```

                           kernel
                          /  ▲
                         /   |
                        /    |
                       ▼     |
                      log    |
                       ▲     |
                        \    |
                         \   |
                    {architecture crate}
                        * x86_64

```

* The `kernel` crate contains platform-independent kernel code and manages the overall control of the kernel. It also provides the kernel interface to userland programs and services.
* The "architecture crate" (e.g. `x86_64`) contains platform-specific kernel code, including the entry to the kernel and memory management code. It initialises the platform, then passes control to the `kernel` crate.
* `log` is used for logging across all kernel crates. The actual logger is created and maintained by the architecture crate.

This entire crate heirachy is compiled into a static library from the architecture crate, and then linked against other kernel objects (depending on platform). This modularity is meant to make it as easy as
possible to extend the kernel to other architectures in the future.
