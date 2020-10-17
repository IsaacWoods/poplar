//! The **Platform Bus** is a concept of a single, abstract "bus" that all devices in the system hang off. These
//! devices are contributed by various **Bus Drivers**, which register devices with the Platfom Bus when they
//! enumerate their respective physical buses. **Device Drivers** can then register interest with the Platform Bus
//! for a specific class of devices using a **Filter**.
//!
//! Devices on the Platform Bus are described by Properties, which provide both generic and platform-specific
//! information. For example, a device created by the PCI bus driver will have `pci.vendor_id`, `pci.device_id`,
//! `pci.class` and `pci.sub_class` as properties. A Device Driver could use the `class` and `subclass` properties
//! to select all PCI devices of a particular type (e.g. useful for a driver for all EHCI controllers), or the
//! `vendor_id` and `device_id` properties to select a specific device (e.g. useful for a graphics driver for a
//! specific graphics card).

#![no_std]

extern crate alloc;

use alloc::{collections::BTreeMap, string::String, vec::Vec};
use libpebble::Handle;
use ptah::{Deserialize, Serialize};

type DeviceName = String;
type PropertyName = String;

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub properties: BTreeMap<PropertyName, Property>,
}

impl DeviceInfo {
    pub fn new(properties: BTreeMap<PropertyName, Property>) -> DeviceInfo {
        DeviceInfo { properties }
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Property {
    Bool(bool),
    Integer(u64),
    String(String),
    MemoryObject(Handle),
}

impl Property {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Property::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_integer(&self) -> Option<u64> {
        match self {
            Property::Integer(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&String> {
        match self {
            Property::String(ref value) => Some(value),
            _ => None,
        }
    }

    pub fn as_memory_object(&self) -> Option<&Handle> {
        match self {
            Property::MemoryObject(ref value) => Some(value),
            _ => None,
        }
    }
}

/// These are messages sent from Bus Drivers to the Platform Bus.
#[derive(Debug, Serialize, Deserialize)]
pub enum BusDriverMessage {
    RegisterDevice(DeviceName, DeviceInfo),
    // TODO: this could have messages to handle hot-plugging (Bus Driver tells Platform Bus a device was removed,
    // we pass that on to the Device Driver if the device was claimed by one)
}

/// These are messages sent from Device Drivers to the Platform Bus.
#[derive(Debug, Serialize, Deserialize)]
pub enum DeviceDriverMessage {
    /// Register interest in a particular type of device. For a device to be managed by this device driver, all of
    /// the `Filter`s must be fulfilled.
    RegisterInterest(Vec<Filter>),
}

/// These are message sent from the Platform Bus to a Device Driver.
#[derive(Debug, Serialize, Deserialize)]
pub enum DeviceDriverRequest {
    /// Request that a Device Driver starts to handle the given Device.
    HandoffDevice(DeviceName, DeviceInfo),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Filter {
    Matches(PropertyName, Property),
}

impl Filter {
    pub fn match_against(&self, properties: &BTreeMap<PropertyName, Property>) -> bool {
        match self {
            Filter::Matches(ref name, ref property) => match properties.get(name) {
                Some(property_to_match) => (property == property_to_match),
                None => false,
            },
        }
    }
}
