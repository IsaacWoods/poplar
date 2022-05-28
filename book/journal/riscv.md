# RISC-V

### Building OpenSBI
OpenSBI is the reference implementation for the Supervisor Binary Interface (SBI). It's basically how you access
M-mode functionality from your S-mode bootloader or kernel.

Firstly, we need a RISC-V C toolchain. On Arch, I installed the `riscv64-unknown-elf-binutils` AUR package. I also
tried to install the `riscv64-unknown-elf-gcc` package, but this wouldn't work, so I built OpenSBI with Clang+LLVM
instead with (from inside `lib/opensbi`):
```
make PLATFORM=generic LLVM=1
```
This can be tested on QEMU with:
```
qemu-system-riscv64 -M virt -bios build/platform/generic/firmware/fw_jump.elf
```

It also seems like you can build with a platform of `qemu/virt` - I'm not sure what difference this makes yet
but guessing it's the hardware it assumes it needs to drive? Worth exploring.

So the jump firmware (`fw_jump.elf`) jumps to a specified address in memory (apparently QEMU can load an ELF which
would be fine initially, but other emulators (specifically SPIKE) can't so other people seem to be using a flat
binary which is kinda icky). Other option would be a payload firmware, which bundles your code into the SBI image
(assuming as a flat binary) and executes it like that.

We should probably make an `xtask` step to build OpenSBI and move it to the `bundled` directory, plus decide what
sort of firmware / booting strategy we're going to use. Then the next step would be some Rust code that can print
to the serial port, to prove it's all working.
