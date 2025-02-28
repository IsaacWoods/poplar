# RISC-V

### Building OpenSBI
OpenSBI is one of the reference implementations for the RISC-V  Supervisor Binary Interface (SBI). SBI provides a
standardized interface for S-mode software (e.g., bootloaders or kernels) to securely access privileged hardware
functionalities through basic features and security mechanisms like isolation and access control, abstracting
M-mode interactions without direct exposure.

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
| Debug         | `0x0`               | `0x100`       |
| MROM          | `0x1000`            | `0x11000`     |
| Test          | `0x100000`          | `0x1000`      |
| CLINT         | `0x0200_0000`       | `0x10000`     |
| PCIe PIO      | `0x0300_0000`       | `0x10000`     |
| PLIC          | `0x0c00_0000`       | `0x4000000`   |
| UART0         | `0x1000_0000`       | `0x100`       |
| Virtio        | `0x1000_1000`       | `0x1000`      |
| Flash         | `0x2000_0000`       | `0x4000000`   |
| PCIe ECAM     | `0x3000_0000`       | `0x10000000`  |
| PCIe MMIO     | `0x4000_0000`       | `0x40000000`  |
| DRAM          | `0x8000_0000`       | `{mem size}`  |

### Getting control from OpenSBI
On QEMU, we can get control from OpenSBI by linking a binary at `0x80200000`, and then using `-kernel` to
automatically load it at the right location. OpenSBI will then jump to this location with the HART ID in `a0` and
a pointer to the device tree in `a1`.

However, this does make setting paging up slightly icky, as has been a problem on other architectures. Basically,
the first binary needs to be linked at a low address with bare translation, and then we need to construct page
tables and enable translation, then jump to a higher address. I'm thinking we might as well do it in two stages:
a Seed stage that loads the kernel and early tasks from the filesystem/network/whatever, builds the kernel page
tables, and then enters the kernel and can be unloaded at a later date. The kernel can then be linked normally at
its high address without faffing around with a bootstrap or anything.

### The device tree
So the device tree seems to be a data structure passed to you that tells you about the hardware present / memory
etc. Hopefully it's less gross than ACPI eh. Repnop has written [a crate, `fdt`](https://docs.rs/fdt/0.1.3/fdt/),
so I think we're just going to use that.

So `fdt` seems to work fine, we can list memory regions etc. The only issue seems to be that `memory_reservations`
doesn't return anything, which is kind of weird. There also seems to be a `/reserved-memory` node, but [this](https://github.com/devicetree-org/devicetree-specification/blob/master/source/chapter3-devicenodes.rst#reserved-memory-and-uefi)
suggests that this doesn't include stuff we want like which part of memory OpenSBI resides in.

[This issue](https://github.com/riscv-software-src/opensbi/issues/70) says Linux just assumes it shouldn't touch
anything before it was loaded. I guess we could use the same approach, reserving the memory used by Seed via linker
symbols, and then seeing where the `loader` device gets put to reserve the ramdisk, but the issue was closed saying
OpenSBI now does the reservations correctly which would be cleaner, but doesn't stack up with what we're seeing.

Ah so actually, `/reserved-memory` does seem to have some of what we need. On QEMU there is one child node, called
`mmode_resv@80000000`, which would fit with being the memory OpenSBI is in. We would still need to handle the
memory we're in, and idk what happens with the `loader` device yet, but it's a start. Might be worth talking to
repnop about whether the crate should use this node.

### Dumb way to load the kernel for now
So for some reason, `fw_cfg` doesn't seem to be working on QEMU 7.1. This is what we were gonna use for loading the
kernel, command line, and user programs, etc. but obvs this is not possible atm. For now, as a workaround, we can
use the `loader` device to load arbitrary data and files into memory.

I'm thinking we could use `0x1_0000_0000` as the base physical address for this - this gives us a maximum of 2GiB
of DRAM, which seems plenty for now (famous last words). We'll need to know the size of the object we're loading on
the guest-side, so we'll load that separately for now (in the future, this whole scheme could be extended to some
sort of mini-filesystem).

Okay so the `loader` device is pretty finnicky, and has no error handling. Turns out you can't define new memory
with it, just load values into RAM, but it doesn't actually tell you this has failed. You then try and read this
on the guest, and get super wierd UB from doing so - it doesn't just fault or whatever, it seems to break code
before you ever read the memory (super weird ngl, didn't stick around to work out what was going on).

Right, seems to be working much better by actually putting the values in RAM. We've extended RAM to 1GiB
(`0x8000_0000..0xc000_0000`) and we'll use this as the new layout:

|    Address    | Description   | Size (bytes) |
|---------------|---------------|--------------|
| 0xb000_0000   | Size of Data  | 4            |
| 0xb000_0004   | Data          | N            |
