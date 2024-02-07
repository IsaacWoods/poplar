#[derive(Clone, Copy, Default, Debug)]
#[repr(C)]
pub struct DeviceDescriptor {
    pub length: u8,
    pub typ: u8,
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

/// The first `8` bytes of a `DeviceDescriptor`.
#[derive(Clone, Copy, Default, Debug)]
#[repr(C)]
pub struct DeviceDescriptorHeader {
    pub length: u8,
    pub typ: u8,
    /// Binary-Coded Decimal representation of the USB Spec version the device supports.
    /// E.g. `2.10` is represented by `0x210`.
    pub bcd_usb: u16,
    pub class: u8,
    pub sub_class: u8,
    pub protocol: u8,
    /// Maximum packet size for endpoint 0 (only 8, 16, 32, and 64 are valid values)
    pub max_control_packet_size: u8,
}
