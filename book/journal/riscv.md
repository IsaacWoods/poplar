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
but guessing it's the hardware it assumes it needs to drive? Worth exploring. (Apparently the `generic` image is
doing dynamic discovery (I'm assuming from the device tree) so that sounds good for now).

So the jump firmware (`fw_jump.elf`) jumps to a specified address in memory (apparently QEMU can load an ELF which
would be fine initially). Other option would be a payload firmware, which bundles your code into the SBI image
(assuming as a flat binary) and executes it like that.

We should probably make an `xtask` step to build OpenSBI and move it to the `bundled` directory, plus decide what
sort of firmware / booting strategy we're going to use. Then the next step would be some Rust code that can print
to the serial port, to prove it's all working.

### QEMU `virt` memory map
Seems everything is memory-mapped, which makes for a nice change coming from x86's nasty port thingy. This is the
`virt` machine's one (from the QEMU source...):

| Region        | Address             | Size          |
|---------------|---------------------|---------------|
| Debug         | 0x0                 | 0x100         |
| MROM          | 0x1000              | 0x11000       |
| Test          | 0x100000            | 0x1000        |
| CLINT         | 0x2000000           | 0x10000       |
| PLIC          | 0xc000000           | 0x4000000     |
| UART0         | 0x10000000          | 0x100         |
| Virtio        | 0x10001000          | 0x1000        |
| Flash         | 0x20000000          | 0x4000000     |
| DRAM          | 0x80000000          | {mem size}    |
| PCIe MMIO     | 0x40000000          | 0x40000000    |
| PCIe PIO      | 0x03000000          | 0x10000       |
| PCIe ECAM     | 0x30000000          | 0x10000000    |
