# MangoPi MQ-Pro
The [MangoPi MQ-Pro](https://mangopi.org/mangopi_mqpro) is a small RISC-V development board, featuring an Allwinner D1 SoC with a single RV64 core, and either 512MiB or 1GiB
of memory. Most public information about the D1 itself can be found on the [Sunxi wiki](https://linux-sunxi.org/D1).

### Boot procedure
The D1 can be booted from an SD card or flash, or, usefully for development, using Allwinner's FEL protocol, which allows data to be loaded into memory and code executed using
a small USB stack. This procedure is best visualised with a diagram:
![Diagram of the D1's boot procedure](../../static/d1_boot_procedure.svg)

The initial part of this process is done by code loaded from the `BROM` (Boot ROM) - it contains the FEL stack, as well as enough code to load the first-stage bootloader from
either an SD card or SPI flash. Data loaded by the FEL stack, or from the bootable media, is loaded into SRAM. The DRAM has to be brought up, either by the first-stage bootloader,
or by a FEL payload.
