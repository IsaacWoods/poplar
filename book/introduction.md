# Introduction
[Poplar](https://github.com/IsaacWoods/poplar) is a general-purpose operating system built around a microkernel and userspace written in Rust.
Drivers and core services that would ordinarily be implemented as part of a traditional monolithic kernel are instead implemented as unprivileged
userspace programs.

Poplar is not a UNIX, and does not aim for binary or source-level compability with existing programs. While this does slow development down significantly,
it gives us the opportunity to design the interfaces we provide from scratch.

Poplar is targeted to run on small computers (think a SoC with a ~1GiB of RAM and a few peripherals) and larger general purpose machines (think a many-core
x86_64 "PC"). It is specifically not designed for small embedded systems - other projects are more suited to this space. Currently, Poplar supports relatively
modern x86_64 and 64-bit RISC-V (RV64GC) machines.