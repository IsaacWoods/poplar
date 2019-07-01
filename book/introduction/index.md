# Introduction
Welcome to the Pebble Book, which serves as the main source of documentation for [Pebble OS](https://github.com/pebble-os).
The Book aims to be both a 10,000-meter overview of Pebble for the interested observer, and a definitive reference for the inner workings of the kernel and userspace. 

Please note that this book (like the rest of the OS!) is still very early in development and not at all complete.
If anything is unclear, please [file an issue](https://github.com/pebble-os/book/issues)!

**Some things documented in this book don't exist in Pebble yet. I use the book as in-progress documentation as I design stuff so I don't have to keep it all in my head.**

### What is Pebble?
At heart, Pebble is a microkernel written in the [Rust programming language](https://rust-lang.org).
Pebble becomes an "OS" when it's combined with other packages such as display and window servers, filesystems and user applications.

Pebble is designed to be a modern microkernel, supporting a minimal system call interface and first-class support for message-passing-based IPC between userspace processes. Versatile message-passing allows
Pebble to move much more out of the kernel than traditionally possible. For example, the kernel has no concept of a filesystem or of files - instead, the VFS and all filesystems are implemented entirely in
userspace, and files are read and written to by passing messages.

### Why Rust?
While Pebble's design is in theory language-agnostic, the implementation is very tied to Rust. Rust is a systems programming language with a rich type system and a novel ownership model that guarantees
memory and thread safety **in safe code**. This qualification is important, as Pebble uses a lot of `unsafe` code out of necessity - it's important to understand that the use of Rust does not in any way
mean that Pebble is automatically bug-free.

However, Rust makes you think a lot more about how to make your programs safe, which is exactly the sort of code we want to be writing for a kernel. This focus on safety, as well as good ergonomics features
and performance, makes Rust perfect for OS-level code.
