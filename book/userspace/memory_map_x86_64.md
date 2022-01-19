# Userspace memory map (x86_64)
x86_64 features an enormous 256TB virtual address space, most of which is available to userspace processes under Poplar. For this reason, things are spread throughout the virtual address space to make it
easy to identify what a virtual address points to.

### Userspace stacks
Within the virtual address space, the userspace stacks are allocated a 4GB range. Each task has a maximum stack size of 2MB, which puts a limit of 2048 tasks per address space.
