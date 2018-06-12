#![no_std]
#![feature(type_ascription)]
#![feature(alloc)]

extern crate alloc;
extern crate bytes_iter;

use alloc::boxed::Box;
use bytes_iter::ByteReader;
use core::mem;
use core::slice;

/// Each node has a unique ID that can be used to identify it. The raw value can be accessed within
/// the kernel.
#[cfg(feature = "kernel")]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeId(pub u16);

/// Each node has a unique ID that can be used to identify it.
#[cfg(not(feature = "kernel"))]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeId(u16);

pub const MAX_PROCESSES: usize = u16::max_value() as usize;

pub struct MessageHeader {
    sender: NodeId,
    receiver: NodeId,
    payload_length: u8,
}

pub struct RawMessage<'a>(&'a [u8]);

impl<'a> RawMessage<'a> {
    /// Interpret the given slice of memory as a message. Unsafe because the other methods of this
    /// type assume the memory does actually point to a message. Extra unsafe because this is a
    /// **potential attack surface** if a malicious userland process gets the kernel to incorrectly
    /// handle a crafted message.
    pub unsafe fn new(address: *const u8, length: usize) -> RawMessage<'a> {
        RawMessage(slice::from_raw_parts(address, length))
    }

    pub fn header(&self) -> Option<MessageHeader> {
        let mut reader = ByteReader::new(self.0.iter());
        Some(MessageHeader {
            sender: NodeId(reader.next_u16()?),
            receiver: NodeId(reader.next_u16()?),
            payload_length: reader.next_u8()?,
        })
    }

    pub fn interpret_as<T>(self) -> Option<Box<T>>
    where
        T: Message,
    {
        const HEADER_LENGTH: usize = mem::size_of::<NodeId>() * 2 + mem::size_of::<u8>();
        let header = self.header()?;

        if self.0.len() - HEADER_LENGTH < header.payload_length as usize {
            return None;
        }

        T::decode(&header, &self.0[HEADER_LENGTH..])
    }
}

/// This is implemented by types that can be passed between nodes as messages. It must be encodable
/// as a series of bytes that is independent from the context in which it was produced (no raw
/// pointers or references)
pub trait Message {
    fn encode<'a>(self) -> &'a [u8];
    fn decode(header: &MessageHeader, payload: &[u8]) -> Option<Box<Self>>
    where
        Self: Sized;
}
