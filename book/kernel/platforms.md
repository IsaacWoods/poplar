# Platforms
A platform is a build target for the kernel. In some cases, there is only one platform for an entire architecture because the hardware is relatively standardized (e.g. x86_64). Other times, hardware is different enough between 
platforms that it's easier to treat them as different targets (e.g. a headless ARM server that boots using UEFI, versus a Raspberry Pi).

### Platform: `x86_64`
The vast majority of x86_64 hardware is pretty similar, and so is treated as a single platform. It uses the `hal_x86_64` HAL. We assume that the platform:
- Boots using UEFI
- Supports the APIC
- Supports the `xsave` instruction

### Platform: `rpi4`
The Raspberry Pi 4. It uses the `hal_arm64` HAL.
