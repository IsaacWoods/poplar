# Debugging the kernel
Kernels can be difficult to debug - this page tries to collect useful techniques for debugging kernels in general,
and also any Poplar specific things that might be useful.

### Using GDB
Firstly, start GDB with (this is just an example, alter e.g. paths as needed):
```
tools/rust_gdb -q "build/Poplar/fat/kernel.elf" -ex "target remote :1234"
```
Note that the `rust_gdb` script is used instead of invoking GDB directly - this installs various plugins to make
life easier.

A few tips for using GDB specific/helpful for kernel debugging:
* QEMU will not run any code (even the firmware) until you run `continue` in GDB. This allows you to place
breakpoints before any code runs.
* By default, the `make gdb` recipe will use KVM acceleration. This means that software breakpoints (created by
`break`) will not work. Use hardware-assisted breakpoints (created with `hbreak`) instead.
* To step through assembly, you must use `si` instead of `s`
* Use `tui enable` to move to the TUI, and then `layout regs` to show both general registers and source

### Emulate with a custom build of QEMU
For particularly tricky issues, it can sometimes be useful to insert `printf`s in QEMU and see if they trigger
when emulating Poplar. The `Makefile` makes this easy - run something like:
```
QEMU_DIR='~/qemu/build/x86_64-softmmu/' make qemu-no-kvm
```
where the location pointed to by `QEMU_DIR` is the build destination of the correct QEMU executable. A lot of the
time, the `printf`s you've inserted will only trigger with TCG, so it's usually best to use `qemu-no-kvm`.

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
