# Summary

- [Introduction](./introduction/index.md)

- [The Kernel](./kernel/index.md)
    - [Platforms](./kernel/platforms/index.md)
        - [MangoPi MQ-Pro](./kernel/platforms/mqpro.md)
    - [Seed](./kernel/seed.md)
    - [Kernel Objects](./kernel/kernel_objects.md)
    - [Debugging the kernel](./kernel/debugging.md)

- [System calls](./syscalls/index.md)
    - [`yield`](./syscalls/yield.md)
    - [`early_log`](./syscalls/early_log.md)
    - [`get_framebuffer`](./syscalls/get_framebuffer.md)
    - [`create_memory_object`](./syscalls/create_memory_object.md)
    - [`map_memory_object`](./syscalls/map_memory_object.md)
    - [`create_channel`](./syscalls/create_channel.md)
    - [`send_message`](./syscalls/send_message.md)
    - [`get_message`](./syscalls/get_message.md)
    - [`register_service`](./syscalls/register_service.md)
    - [`subscribe_to_service`](./syscalls/subscribe_to_service.md)
    - [`pci_get_info`](./syscalls/pci_get_info.md)

- [Userspace](./userspace/index.md)
    - [Capabilities](./userspace/capabilities.md)
    - [Memory map (x86_64)](./userspace/memory_map_x86_64.md)
    - [Platform Bus](./userspace/platform_bus.md)

- [Message Passing](./message_passing/index.md)
    - [Ptah wire format](./message_passing/wire_format.md)

- [Journal](./journal/index.md)
    - [Building a `rustc` target for Poplar](./journal/rustc_target.md)
    - [USB](./journal/usb.md)
    - [RISC-V](./journal/riscv.md)
    - [PCI interrupt routing](./journal/pci_interrupt_routing.md)
