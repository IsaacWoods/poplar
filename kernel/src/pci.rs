use alloc::collections::BTreeMap;
use pci_types::{Bar, BaseClass, DeviceId, DeviceRevision, Interface, PciAddress, SubClass, VendorId, MAX_BARS};

pub struct PciDevice {
    pub vendor_id: VendorId,
    pub device_id: DeviceId,
    pub revision: DeviceRevision,
    pub class: BaseClass,
    pub sub_class: SubClass,
    pub interface: Interface,
    pub bars: [Option<Bar>; MAX_BARS],
}

pub struct PciInfo {
    pub devices: BTreeMap<PciAddress, PciDevice>,
}
