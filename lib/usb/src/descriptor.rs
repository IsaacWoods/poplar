use crate::hid::HidDescriptor;
use core::mem;
use ptah::{Deserialize, Serialize};

/// Used to 'visit' each descriptor within a configuration hierachy. Used with
/// [`walk_configuration`].
pub trait ConfigurationVisitor {
    fn visit_configuration(&mut self, descriptor: &ConfigurationDescriptor) {
        let _ = descriptor;
    }
    fn visit_interface(&mut self, descriptor: &InterfaceDescriptor) {
        let _ = descriptor;
    }
    fn visit_endpoint(&mut self, descriptor: &EndpointDescriptor) {
        let _ = descriptor;
    }
    fn visit_hid(&mut self, descriptor: &HidDescriptor) {
        let _ = descriptor;
    }
    fn visit_other(&mut self, descriptor_typ: u8, bytes: &[u8]) {
        let _ = (descriptor_typ, bytes);
    }
}

/// Walk a configuration descriptor hierachy for a device. This should be passed the bytes received
/// after requesting a full Configuration descriptor from a device, and will call the passed
/// [`ConfigurationVisitor`] for each contained descriptor.
///
/// A conformant device will have a Configuration Descriptor, followed by Interface Descriptors,
/// which may potentially each have one or more Endpoint Descriptors. Endpoint descriptors pertain
/// to the Interface descriptor that they follow. Class and vendor-specific descriptors can be
/// interleaved with the standard descriptors - [`ConfigurationVisitor::visit_other`] will be called
/// for each of them.
pub fn walk_configuration(bytes: &[u8], visitor: &mut impl ConfigurationVisitor) {
    fn at<T>(bytes: &[u8], offset: usize) -> &T {
        assert!(offset + mem::size_of::<T>() <= bytes.len());
        unsafe { &*(bytes.as_ptr().byte_add(offset) as *const T) }
    }

    // Start with the configuration descriptor at the start
    let configuration_descriptor = at::<ConfigurationDescriptor>(bytes, 0);
    visitor.visit_configuration(configuration_descriptor);

    // Then any subsequent descriptors
    let mut offset = configuration_descriptor.length as usize;
    while offset < configuration_descriptor.total_length as usize {
        let base = at::<DescriptorBase>(bytes, offset);

        match base.typ {
            4 => {
                let interface_descriptor = at::<InterfaceDescriptor>(bytes, offset);
                visitor.visit_interface(interface_descriptor);
            }
            5 => {
                let endpoint_descriptor = at::<EndpointDescriptor>(bytes, offset);
                visitor.visit_endpoint(endpoint_descriptor);
            }
            33 => {
                let hid_descriptor = at::<HidDescriptor>(bytes, offset);
                visitor.visit_hid(hid_descriptor);
            }
            other => {
                let descriptor_bytes = &bytes[offset..(offset + base.length as usize)];
                visitor.visit_other(other, descriptor_bytes);
            }
        }
        offset += base.length as usize;
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug, Serialize, Deserialize)]
#[repr(u8)]
pub enum DescriptorType {
    #[default]
    _Reserved = 0,
    Device = 1,
    Configuration = 2,
    String = 3,
    Interface = 4,
    Endpoint = 5,
    DeviceQualifier = 6,
    OtherSpeedConfiguration = 7,
    InterfacePower = 8,
    /*
     * TODO: I'm not sure if we want non-standard descriptors to be in this enum or no
     */
    Hid = 33,
    Report = 34,
}

#[repr(C)]
pub struct DescriptorBase {
    pub length: u8,
    /*
     * XXX: we don't use `DescriptorType` here to allow this to correctly represent class and
     * vendor specific descriptors
     */
    pub typ: u8,
}

#[derive(Clone, Copy, Default, Debug)]
#[repr(C)]
pub struct DeviceDescriptor {
    pub length: u8,
    pub typ: DescriptorType,
    /// Binary-Coded Decimal representation of the USB Spec version the device supports.
    /// E.g. `2.10` is represented by `0x210`.
    pub bcd_usb: u16,
    pub class: u8,
    pub sub_class: u8,
    pub protocol: u8,
    /// Maximum packet size for endpoint 0 (only 8, 16, 32, and 64 are valid values)
    pub max_control_packet_size: u8,
    pub vendor_id: u16,
    pub product_id: u16,
    pub bcd_device: u16,
    /// Index of string descriptor describing the device's manufacturer.
    pub manufacturer_index: u8,
    pub product_index: u8,
    pub serial_number: u8,
    pub num_configurations: u8,
}

/// A configuration descriptor describes a particular configuration of a USB device. The
/// value of the `configuration_value` field can be passed to a device within a `SetConfiguration`
/// request to make the device assume that configuration.
#[derive(Clone, Copy, Default, Debug)]
#[repr(C)]
pub struct ConfigurationDescriptor {
    pub length: u8,
    pub typ: DescriptorType,
    pub total_length: u16,
    pub num_interfaces: u8,
    pub configuration_value: u8,
    pub configuration_index: u8,
    pub attributes: ConfigurationAttributes,
    /// The maximum power consumption of the device in this configuration, expressed in 2mA units.
    pub max_power: u8,
}

mycelium_bitfield::bitfield! {
    #[derive(Default)]
    pub struct ConfigurationAttributes<u8> {
        const _RESERVED0 = 5;
        pub const REMOTE_WAKEUP: bool;
        pub const SELF_POWERED: bool;
        const _RESERVED1 = 1;
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct InterfaceDescriptor {
    pub length: u8,
    pub typ: DescriptorType,
    pub interface_num: u8,
    pub alternate_setting: u8,
    pub num_endpoints: u8,
    pub interface_class: u8,
    pub interface_subclass: u8,
    pub interface_protocol: u8,
    pub interface_index: u8,
}

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct EndpointDescriptor {
    pub length: u8,
    pub typ: DescriptorType,
    pub endpoint_address: EndpointAddress,
    pub attributes: EndpointAttributes,
    pub max_packet_size: u16,
    pub interval: u8,
}

mycelium_bitfield::bitfield! {
    pub struct EndpointAddress<u8> {
        pub const NUMBER = 4;
        pub const _RESERVED0 = 3;
        /// `true` for IN endpoints, `false` for OUT endpoints
        pub const DIRECTION: bool;
    }
}

mycelium_bitfield::bitfield! {
    pub struct EndpointAttributes<u8> {
        pub const TRANFER_TYPE: TransferType;
        pub const SYNCH_TYPE: SynchType;
        pub const USAGE_TYPE: UsageType;
    }
}

mycelium_bitfield::enum_from_bits! {
    #[derive(PartialEq, Eq, Debug)]
    pub enum TransferType<u8> {
        Control = 0b00,
        Isochronous = 0b01,
        Bulk = 0b10,
        Interrupt = 0b11,
    }
}

mycelium_bitfield::enum_from_bits! {
    #[derive(Debug)]
    pub enum SynchType<u8> {
        None = 0b00,
        Asynchronous = 0b01,
        Adaptive = 0b10,
        Synchronous = 0b11,
    }
}

mycelium_bitfield::enum_from_bits! {
    #[derive(Debug)]
    pub enum UsageType<u8> {
        Data = 0b00,
        Feedback = 0b01,
        ImplicitFeedbackData = 0b10,
        _Reserved = 0b11,
    }
}
