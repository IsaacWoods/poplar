# Seed
Seed is Poplar's bootloader ± pre-kernel. What it is required to do varies by platform, but generally it is
responsible for bringing up the system, loading the kernel and initial tasks into memory, and preparing the
environment for executing the kernel.

### `x86_64`
On `x86_64`, Seed is an UEFI executable that utilises boot services to load the kernel and initial tasks. The Seed
exectuable, the kernel, and other files are all held in the EFI System Partition (ESP) - a FAT filesystem present
in all UEFI-booted systems.

### `riscv`
On RiscV, Seed is more of a pre-kernel than a traditional bootloader. It is booted into by the system firmware, and
then has its own set of drivers to load the kernel and other files from the correct filesystem, or elsewhere.

**The boot mechanism has not yet been fully designed for RiscV, and also will heavily depend on the hardware
target, as booting different platforms is much less standardised than on x86_64.**
