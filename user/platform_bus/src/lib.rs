#![no_std]

extern crate alloc;

use alloc::{collections::BTreeMap, string::String};
use serde::{Deserialize, Serialize};

type DeviceName = String;
type PropertyName = String;

/// A `Device` represents some abstract piece of hardware on the platform, usually on some type of bus. They are
/// created by a *Bus Driver*, and can be consumed by a *Device Driver*. Properties describe the device, both
/// generally and in platform-specific ways, to provide information to device drivers. For example, a device
/// created by the PCI bus driver will have `pci.vendor_id`, `pci.device_id`, `pci.class` and `pci.sub_class` as
/// properties.
#[derive(Serialize, Deserialize, Debug)]
pub struct Device {
    properties: BTreeMap<PropertyName, Property>,
}

impl Device {
    pub fn new(properties: BTreeMap<PropertyName, Property>) -> Device {
        Device { properties }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Property {
    Bool(bool),
    Integer(u64),
    String(String),
}

/// These are messages sent from Bus Drivers to the Platform Bus.
#[derive(Serialize, Deserialize, Debug)]
pub enum BusDriverMessage {
    RegisterDevice(DeviceName, Device),
    AddProperty(PropertyName, Property),
    RemoveProperty(PropertyName),
    // TODO: this could have messages to handle hot-plugging (Bus Driver tells Platform Bus a device was removed,
    // we pass that on to the Device Driver if the device was claimed by one)
}
