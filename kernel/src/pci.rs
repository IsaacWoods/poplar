use alloc::collections::BTreeMap;
use pci_types::{BaseClass, DeviceId, DeviceRevision, Interface, PciAddress, SubClass, VendorId};

pub struct PciDevice {
    pub vendor_id: VendorId,
    pub device_id: DeviceId,
    pub revision: DeviceRevision,
    pub class: BaseClass,
    pub sub_class: SubClass,
    pub interface: Interface,
}

pub struct PciInfo {
    pub devices: BTreeMap<PciAddress, PciDevice>,
}
