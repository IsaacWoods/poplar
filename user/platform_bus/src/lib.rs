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
//!
//! Sometimes, a Device Driver will need to inspect a device to know whether it can drive it. A
//! driver may use a more permissive filter to attract devices it may be able to drive, and then
//! filter them by replying to `QuerySupport` messages from the Platform Bus. Device Drivers that
//! can provide an exact filter for the devices they can drive can safely blindly return `true` to
//! these queries.

pub mod input;

use poplar::interrupt::Interrupt;
use ptah::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    poplar::{event::Event, Handle},
};

type DeviceName = String;
type PropertyName = String;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceInfo(pub BTreeMap<PropertyName, Property>);

#[derive(Debug, Serialize, Deserialize)]
pub struct HandoffInfo(pub BTreeMap<PropertyName, HandoffProperty>);

impl DeviceInfo {
    pub fn get_as_bool(&self, name: &str) -> Option<bool> {
        self.0.get(name)?.as_bool()
    }

    pub fn get_as_integer(&self, name: &str) -> Option<u64> {
        self.0.get(name)?.as_integer()
    }

    pub fn get_as_str(&self, name: &str) -> Option<&str> {
        self.0.get(name)?.as_str()
    }

    pub fn get_as_bytes(&self, name: &str) -> Option<&[u8]> {
        self.0.get(name)?.as_bytes()
    }
}

impl HandoffInfo {
    pub fn get_as_bool(&self, name: &str) -> Option<bool> {
        self.0.get(name)?.as_bool()
    }

    pub fn get_as_integer(&self, name: &str) -> Option<u64> {
        self.0.get(name)?.as_integer()
    }

    pub fn get_as_str(&self, name: &str) -> Option<&str> {
        self.0.get(name)?.as_str()
    }

    pub fn get_as_bytes(&self, name: &str) -> Option<&[u8]> {
        self.0.get(name)?.as_bytes()
    }

    pub fn get_as_memory_object(&self, name: &str) -> Option<Handle> {
        self.0.get(name)?.as_memory_object()
    }

    pub fn get_as_event(&self, name: &str) -> Option<Event> {
        self.0.get(name)?.as_event()
    }

    pub fn get_as_interrupt(&self, name: &str) -> Option<Interrupt> {
        self.0.get(name)?.as_interrupt()
    }

    pub fn get_as_channel(&self, name: &str) -> Option<Handle> {
        self.0.get(name)?.as_channel()
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Property {
    Bool(bool),
    Integer(u64),
    String(String),
    Bytes(Vec<u8>),
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

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Property::String(ref value) => Some(value),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Property::Bytes(ref value) => Some(value),
            _ => None,
        }
    }
}

#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum HandoffProperty {
    Bool(bool),
    Integer(u64),
    String(String),
    Bytes(Vec<u8>),
    MemoryObject(Handle),
    Event(Handle),
    Interrupt(Handle),
    Channel(Handle),
}

impl HandoffProperty {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            HandoffProperty::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_integer(&self) -> Option<u64> {
        match self {
            HandoffProperty::Integer(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            HandoffProperty::String(ref value) => Some(value),
            _ => None,
        }
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            HandoffProperty::Bytes(ref value) => Some(value),
            _ => None,
        }
    }

    pub fn as_memory_object(&self) -> Option<Handle> {
        match self {
            HandoffProperty::MemoryObject(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_event(&self) -> Option<Event> {
        match self {
            HandoffProperty::Event(value) => Some(Event::new_from_handle(*value)),
            _ => None,
        }
    }

    pub fn as_interrupt(&self) -> Option<Interrupt> {
        match self {
            HandoffProperty::Interrupt(value) => Some(Interrupt::new_from_handle(*value)),
            _ => None,
        }
    }

    pub fn as_channel(&self) -> Option<Handle> {
        match self {
            HandoffProperty::Channel(value) => Some(*value),
            _ => None,
        }
    }
}

/// These are messages sent from Bus Drivers to the Platform Bus.
#[derive(Debug, Serialize, Deserialize)]
pub enum BusDriverMessage {
    RegisterDevice(DeviceName, DeviceInfo, HandoffInfo),
    // TODO: this could have messages to handle hot-plugging (Bus Driver tells Platform Bus a device was removed,
    // we pass that on to the Device Driver if the device was claimed by one)
}

/// These are messages sent from Device Drivers to the Platform Bus.
#[derive(Debug, Serialize, Deserialize)]
pub enum DeviceDriverMessage {
    /// Register interest in a particular type of device. For a device to be managed by this device driver, all of
    /// the `Filter`s must be fulfilled.
    RegisterInterest(Vec<Filter>),
    /// Response to a `QuerySupport` request, indicating that this Device Driver either can or
    /// cannot drive the specified device.
    CanSupport(DeviceName, bool),
}

/// These are message sent from the Platform Bus to a Device Driver.
#[derive(Debug, Serialize, Deserialize)]
pub enum DeviceDriverRequest {
    /// Query whether a Device Driver can drive the specified device. Respond with a `CanSupport`
    /// message.
    QuerySupport(DeviceName, DeviceInfo),
    /// Request that a Device Driver starts to handle the given Device.
    HandoffDevice(DeviceName, DeviceInfo, HandoffInfo),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Filter {
    Matches(PropertyName, Property),
    All(Vec<Filter>),
}

impl Filter {
    pub fn match_against(&self, properties: &BTreeMap<PropertyName, Property>) -> bool {
        match self {
            Filter::Matches(ref name, ref property) => match properties.get(name) {
                Some(property_to_match) => property == property_to_match,
                None => false,
            },
            Filter::All(filters) => filters
                .iter()
                .fold(true, |matches_so_far, filter| matches_so_far && filter.match_against(&properties)),
        }
    }
}

/// Type returned by a query to the PlatformBus's inspection service. This is designed to be used
/// from a shell/console to query the state of the PlatformBus and its devices.
/*
 * TODO: including all the properties of each device currently causes a stack overflow in
 * `platform_bus`. I haven't yet worked out if this is a true overflow (it shouldn't be that large
 * on the stack), or something else going on. It is still included now as a POC of an interaction
 * between a service-providing task and `fb_console`.
 */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlatformBusInspect {
    pub devices: Vec<DeviceInspect>,
    pub bus_drivers: Vec<BusDriverInspect>,
    pub device_drivers: Vec<DeviceDriverInspect>,
}

// TODO: maybe include names of bus/bus+device drivers (will require a lookup for each)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DeviceInspect {
    Unclaimed {
        name: String,
        device_info: BTreeMap<PropertyName, Property>,
        // handoff_info_names: Vec<PropertyName>,
    },
    Claimed {
        name: String,
        device_info: BTreeMap<PropertyName, Property>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BusDriverInspect {
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceDriverInspect {
    pub name: String,
    pub filters: Option<Vec<Filter>>,
}
