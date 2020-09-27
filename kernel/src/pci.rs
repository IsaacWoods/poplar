use alloc::collections::BTreeMap;
use pci_types::{BaseClass, DeviceId, DeviceRevision, Interface, PciAddress, SubClass, VendorId};

pub struct PciDevice {
    pub vendor_id: VendorId,
    pub device_id: DeviceId,
}

pub struct PciInfo {
    pub devices: BTreeMap<PciAddress, PciDevice>,
}
