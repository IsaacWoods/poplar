#![no_std]

extern crate alloc;

pub mod descriptor;
pub mod hid;
pub mod setup;

use alloc::vec::Vec;
use descriptor::DescriptorType;
use ptah::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum EndpointDirection {
    In,
    Out,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DeviceControlMessage {
    UseConfiguration(u8),
    UseInterface(u8, u8),
    OpenEndpoint { number: u8, direction: EndpointDirection, max_packet_size: u16 },
    GetInterfaceDescriptor { typ: DescriptorType, index: u8, length: u16 },
    InterruptTransferIn { endpoint: u8, packet_size: u16 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DeviceResponse {
    Data(Vec<u8>),
    NoData,
    Descriptor { typ: DescriptorType, index: u8, bytes: Vec<u8> },
}
