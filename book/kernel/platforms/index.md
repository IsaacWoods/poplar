# Platforms
A platform is a build target for the kernel. In some cases, there is only one platform for an entire architecture because the hardware is relatively standardized (e.g. x86_64).
Other times, hardware is different enough between platforms that it's easier to treat them as different targets (e.g. a headless ARM server that boots using UEFI, versus a
Raspberry Pi).

All supported platforms are enumerated in the table below - some have their own sections with more details, while others are just described below. The platform you want to
build for is specified in your `Poplar.toml` configuration file, or with the `-p`/`--platform` flag to `xtask`. Some platforms also have custom `xtask` commands to, for
example, flash a device with a built image.

| Platform name                    | Arch    | Description                             |
|----------------------------------|---------|-----------------------------------------|
| `x64`                            | x86_64  | Modern x86_64 platform.                 |
| `rv64_virt`                      | RV64    | A virtual RISC-V QEMU platform.         |
| [`mq_pro`](./mqpro.md)           | RV64    | The MangoPi MQ-Pro RISC-V platform.     |

### Platform: `x64`
The vast majority of x86_64 hardware is pretty similar, and so is treated as a single platform. It uses the `hal_x86_64` HAL. We assume that the platform:
- Boots using UEFI (using `seed_uefi`)
- Supports the APIC
- Supports the `xsave` instruction

### Platform: `rv64_virt`
This is a virtual RISC-V platform emulated by `qemu-system-riscv64`'s `virt` machine. It features:
- A customizable number of emulated RV64 HARTs
- Is booted via QEMU's `-kernel` option and QEMU BIOS firmware
- A Virtio block device with attached GPT 'disk'
- Support for USB devices via EHCI

Devices such as the EHCI USB controller are connected to a PCIe bus, and so we use the [Advanced Interrupt Architecture](https://github.com/riscv/riscv-aia)
with MSIs to avoid the complexity of shared pin-based PCI interrupts. This is done by passing the `aia=aplic-imsic` machine option to QEMU.
