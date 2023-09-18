# Debugging the kernel
Kernels can be difficult to debug - this page tries to collect useful techniques for debugging kernels in general,
and also any Poplar specific things that might be useful.

### Poplar specific: the breakpoint exception
The breakpoint exception is useful for inspecting the contents of registers at specific points, such as in sections
of assembly (where it's inconvenient to call into Rust, or to use a debugger because getting `global_asm!` to play
nicely with GDB is a pain).

Simply use the `int3` instruction:
```
...

mov rsp, [gs:0x10]
int3  // Is my user stack pointer correct?
sysretq
```

### Building OVMF
Building a debug build of OVMF isn't too hard (from the base of the `edk2` repo):
```
OvmfPkg/build.sh -a X64
```

By default, debug builds of OVMF will output debugging information on the ISA `debugcon`, which is actually
probably nicer for our purposes than most builds, which pass `DEBUG_ON_SERIAL_PORT` during the build. To log the
output to a file, you can pass `-debugcon file:ovmf_debug.log -global isa-debugcon.iobase=0x402` to QEMU.
