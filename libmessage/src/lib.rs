#![no_std]
#![feature(type_ascription)]
#![feature(integer_atomics)]

extern crate bytes_iter;
extern crate serde;
#[macro_use]
extern crate serde_derive;

pub mod serializer;
pub mod kernel;
pub mod buffers;
mod format;

use core::fmt::Display;
use bytes_iter::ByteReader;
use core::slice;
use serde::{Serialize, Deserialize};

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

#[repr(C, packed)]
pub struct MessageHeader {
    destination: NodeId,
    payload_length: u8,
}

#[derive(Debug)]
pub enum Error {
    SendBufferFull,
}

impl ::serde::ser::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        unimplemented!();
    }
}

impl ::serde::de::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        unimplemented!();
    }
}

impl Display for Error {
    fn fmt(&self, formatter: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
        unimplemented!();
        // formatter.write_str(::core::error::Error::description(self))
    }
}

pub type Result<T> = ::core::result::Result<T, Error>;

// TODO: do we need this type any more?
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
            destination: NodeId(reader.next_u16()?),
            payload_length: reader.next_u8()?,
        })
    }

    // pub fn interpret_as<'de, T>(self) -> Option<Box<T>>
    // where
    //     T: Message<'de>,
    // {
    //     const HEADER_LENGTH: usize = mem::size_of::<NodeId>() * 2 + mem::size_of::<u8>();
    //     let header = self.header()?;

    //     if self.0.len() - HEADER_LENGTH < header.payload_length as usize {
    //         return None;
    //     }

    //     T::decode(&header, &self.0[HEADER_LENGTH..])
    // }
}

/// This is implemented by types that can be passed between nodes as messages. It must be encodable
/// as a series of bytes that is independent from the context in which it was produced (no raw
/// pointers or references)
// TODO: should we use SerializeOwned or Serialize<'de> where 'de is the lifetime of the data
// within the raw message? How long do messages stay in the receive-buffer (until they're dropped,
// maybe (how can we implement a custom `drop` on all messages tho? (custom derive could do it)))
pub trait Message<'de>: Serialize + Deserialize<'de> { }

pub trait MessageWriter {
    fn write_u8(&mut self, value: u8) -> Result<()>;

    fn write_u16(&mut self, value: u16) -> Result<()> {
        self.write_u8((value & 0xff) as u8)?;
        self.write_u8(((value >> 8) & 0xff) as u8)?;
        Ok(())
    }

    fn write_u32(&mut self, value: u32) -> Result<()> {
        self.write_u16((value & 0xff_ff) as u16)?;
        self.write_u16(((value >> 16) & 0xff_ff) as u16)?;
        Ok(())
    }

    fn write_u64(&mut self, value: u64) -> Result<()> {
        self.write_u32((value & 0xff_ff_ff_ff) as u32)?;
        self.write_u32(((value >> 32) & 0xff_ff_ff_ff) as u32)?;
        Ok(())
    }
}

pub trait MessageReader {
    fn read_u8(&self) -> Result<u8>;
    fn read_u16(&self) -> Result<u16>;
    fn read_u32(&self) -> Result<u32>;
    fn read_u64(&self) -> Result<u64>;
}
