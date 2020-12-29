# USB
USB has a **host** that sends requests to **devices** (devices only respond when asked something). Some devices are
**dual role devices (DRD)** (previously called On-The-Go (OTG) devices), and can dynamically negotiate whether
they're the host or the device.

Each device can have one ore more **interfaces**, which each have one or more **endpoints**. Each endpoint has a
hardcoded direction (host-to-device or device-to-host). There are a few types of endpoint (the type is decided
during interface configuration):
- **Control endpoints** are for configuration and control requests
- **Bulk endpoints** are for bulk transfers
- **Isochronous endpoints** are for periodic transfers with a reserved bandwidth
- **Int endpoints** are for transfers triggered by interruptions

The interfaces and endpoints a device has are described by descriptors reported by the device during configuration.

Every device has a special endpoint called `ep0`. It's an in+out control endpoint, and is used to configure the
other endpoints.
