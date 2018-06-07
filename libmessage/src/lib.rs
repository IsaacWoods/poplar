#![no_std]

/// Each node has a unique ID that can be used to identify it.
pub struct NodeId(u16);

pub const MAX_PROCESSES: usize = u16::max_value() as usize;

#[repr(C, packed)]
pub struct MessageHeader {
    sender: NodeId,
    receiver: NodeId,
    payload_length: u8,
}

pub trait Message {
    fn encode<'a>(self) -> &'a [u8];
}
