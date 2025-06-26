# Platform Bus
The Platform Bus, or PBus, is a core component of most Poplar systems. It has components that live in kernelspace
and in userspace. It manages an abstract "bus" of devices that represent either physical or abstract devices that are part of
the platform (e.g. part of a SoC) or that are plugged in as peripherals.

Devices are managed by **bus drivers**, which can be implemented as part of the kernel or in userspace, and can be used by
userspace **device drivers**. Communication between drivers takes place through `Channel` kernel objects.

### Why does a microkernel have kernel bus drivers?
While Poplar is at its core a microkernel, it is pragmatic for the kernel to manage certain busses and devices. It, for example,
performs the initial bringup of ACPI and of the PCI bus.

Rationale for this depends on the particular driver. Some may require access to a resource that has not been exposed to userspace,
such as the x86 I/O address space - while we could implement an x86 RTC userspace driver that has permission to perform CMOS memory
reads, I remain unconvinced that this adds safety and security over a traditional kernelspace driver.

In essence, even a microkernel retains responsibility for ensuring fair and correct multiplexing of system resources between userspace
services and applications. In my view, this includes things like device power management, and as such, implementation of ACPI and similar
functionality in kernelspace does not contradict my personal ethos for microkernel design.

### Device representation on the Platform Bus
Devices on the Platform Bus can be quite abstract, or represent literal devices that are part of the platform, or plugged in as peripherals. Examples
include a framebuffer "device" provided by a driver for a graphics-capable device, a power-management chip built into the platform, and USB devices,
respectively.

Devices are described by a series of properties, which are typed pieces of data associated with a label. **Device properties** are used to identify
devices, and are given to every device driver that claims it may be able to drive a device. **Handoff properties** are only transfered to a driver
once it has been selected to drive a device, and can contain handles to kernel objects needed to drive the device. These handles are transferred
to the task implementing the device driver, which is why they cannot be send arbitrarily to drivers to query support.

### Device registration
TODO

### Device hand-off to device driver
TODO

### Standard devices
The Platform Bus library defines expected properties and behaviour for a number of standard device classes, in an attempt to increase compatability
across drivers and device users. Additional properties may be added as necessary for an individual device.

#### PCI devices
Platform Bus will use information provided by the kernel to create devices for each enumerated PCI device. Standard properties:
| Property              | Type          | Description                                                                       |
|-----------------------|---------------|-----------------------------------------------------------------------------------|
| `pci.vendor_id`       | Integer       | Vendor ID of the PCI device                                                       |
| `pci.device_id`       | Integer       | Device ID of the PCI device                                                       |
| `pci.class`           | Integer       | Class of the PCI device                                                           |
| `pci.sub_class`       | Integer       | Sub-class of the PCI device                                                       |
| `pci.interface`       | Integer       | Interface of the PCI device                                                       |
| `pci.interrupt`       | Event         | If configured, an `Event` that is triggered when the PCI device gets an IRQ       |
| `pci.barN.size`       | Integer       | `N` is a number from 0-6. The size of the given BAR, if present.                  |
| `pci.barN.handle`     | MemoryObject  | `N` is a number from 0-6. A memory object mapped to the given BAR, if present.    |

Generally, specific devices (e.g. a specific GPU) can be detected with a combination of the `vendor_id` and `device_id` properties, while a type of
device can be identified via the `class`, `sub_class`, and `interface` properties. Drivers should filter against the appropriate properties depending
on the devices they can drive.

#### USB devices
USB devices may be added to the Platform Bus by a USB Host Controller driver, and can be consumed by a wide array of drivers.
Standard properties:
| Property              | Type          | Description                                                                       |
|-----------------------|---------------|-----------------------------------------------------------------------------------|
| `usb.vendor_id`       | Integer       | Class of the USB device                                                           |
| `usb.product_id`      | Integer       | Class of the USB device                                                           |
| `usb.class`           | Integer       | Class of the USB device                                                           |
| `usb.sub_class`       | Integer       | Sub-class of the USB device                                                       |
| `usb.protocol`        | Integer       | Protocol of the USB device                                                        |
| `usb.config0`         | Bytes         | Byte-stream of the first configuration descriptor of the device                   |
| `usb.channel`         | Channel       | Control channel to configure and control the device via the bus driver            |

#### HID devices
TODO
