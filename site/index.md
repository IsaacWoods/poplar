---
layout: default
---

Pebble is a microkernel and userspace written in Rust. It is still early in development.

* Pebble is **not** a UNIX
* Processes talk to the kernel through a very minimal system call interface
* Processes communicate with each other through message passing facilitated by the kernel
* Drivers live in userspace

The best place to learn more about Pebble is [the book](https://pebble-os.github.io/book).
