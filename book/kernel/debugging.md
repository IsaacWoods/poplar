# Debugging the kernel
Kernels can be difficult to debug - this page tries to collect useful techniques for debugging kernels in general,
and also any Pebble specific things that might be useful.

### Using GDB
QEMU can communicate with GDB using the remote target protocol. Running `make gdb` will start QEMU in remote
debugging mode, and then start a version of GDB connected to the QEMU instance, with the kernel ELF preloaded and
Rust plugins installed.

A few tips for using GDB specific/helpful for kernel debugging:
* QEMU will not run any code (even the firmware) until you run `continue` in GDB. This allows you to place
breakpoints before any code runs.
* By default, the `make gdb` recipe will use KVM acceleration. This means that software breakpoints (created by
`break`) will not work. Use hardware-assisted breakpoints (created with `hbreak`) instead.
* To step through assembly, you must use `si` instead of `s`
* Use `tui enable` to move to the TUI, and then `layout regs` to show both general registers and source

### Emulate with a custom build of QEMU
For particularly tricky issues, it can sometimes be useful to insert `printf`s in QEMU and see if they trigger
when emulating Pebble. The `Makefile` makes this easy - run something like:
```
QEMU_DIR='~/qemu/build/x86_64-softmmu/' make qemu-no-kvm
```
where the location pointed to by `QEMU_DIR` is the build destination of the correct QEMU executable. A lot of the
time, the `printf`s you've inserted will only trigger with TCG, so it's usually best to use `qemu-no-kvm`.

### Pebble specific: the breakpoint exception
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
