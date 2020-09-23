use alloc::collections::BTreeMap;
use hal::pci::{DeviceId, PciAddress, VendorId};

pub struct PciDevice {
    pub vendor_id: VendorId,
    pub device_id: DeviceId,
}

pub struct PciInfo {
    pub devices: BTreeMap<PciAddress, PciDevice>,
}
