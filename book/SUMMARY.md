# Summary

- [Introduction](./introduction/index.md)

- [The Kernel](./kernel/index.md)
    - [Platforms](./kernel/platforms.md)
    - [Efiloader](./kernel/efiloader.md)
    - [Kernel Objects](./kernel/kernel_objects.md)
    - [System calls](./kernel/syscalls.md)
    - [Debugging the kernel](./kernel/debugging.md)

- [Syscalls](./syscalls/index.md)
    - [`yield`](./syscalls/yield.md)
    - [`early_log`](./syscalls/early_log.md)
    - [`get_framebuffer`](./syscalls/get_framebuffer.md)
    - [`map_memory_object`](./syscalls/map_memory_object.md)
    - [`create_channel`](./syscalls/create_channel.md)
    - [`send_message`](./syscalls/send_message.md)
    - [`register_service`](./syscalls/register_service.md)
    - [`subscribe_to_service`](./syscalls/subscribe_to_service.md)

- [Userspace](./userspace/index.md)
    - [Capabilities](./userspace/capabilities.md)
    - [Memory map (x86_64)](./userspace/memory_map_x86_64.md)

- [Message Passing](./message_passing/index.md)
    - [Message Format](./message_passing/fmt.md)
