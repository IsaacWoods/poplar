# PCI interrupt routing
PCI interrupt routing is the process of working out which platform-specific interrupt will fire when
a given PCI device issues an interrupt. In general, we use message-signalled interrupts (MSIs) where
avaiable, and fall back to the legacy interrupt pins (INTA, INTB, INTC, INTD) on devices where they are
not.

### Legacy interrupt pins
- Each function header contains an interrupt pin field that can be `0` (no interrupts), or `1`
  through `4` for each pin.
- Interrupts from devices that share a pin cannot be differentiated from each other without querying
  the devices themselves. For us, this means usermode drivers will need to be awoken before knowing
  their device has actually received an interrupt.

The pin used by each device is not programmable - each device hardcodes it at time of manufacture. However,
they can be remapped by any bridge between the device and host, so that the interrupt signal on the
upstream side of the bridge differs to the device's reported interrupt pin. This was necessitated
by manufacturers defaulting to using INTA - usage of the 4 available pins was unbalanced, so
firmware improves performance by rebalancing them at the bridge.

How the pins have been remapped is communicated to the operating system via a platform-specific
mechanism. On modern x86 systems, this is through the `_PRT` method in the ACPI namespace
(before ACPI, BIOS methods and later MP tables were used). On ARM and RISC-V, the device tree
specifies this mapping through the `interrupt-map` property on the platform's interrupt controllers.
