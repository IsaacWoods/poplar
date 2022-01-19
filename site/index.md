---
layout: default
---

Poplar is a microkernel and userspace written in Rust. It is still early in development.

* Poplar is **not** a UNIX
* Processes talk to the kernel through a very minimal system call interface
* Processes communicate with each other through message passing facilitated by the kernel
* Drivers live in userspace

The best place to learn more about Poplar is [the book](https://isaacwoods.github.io/poplar/book).
