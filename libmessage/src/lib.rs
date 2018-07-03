#![no_std]
#![feature(type_ascription)]
#![feature(integer_atomics)]

extern crate serde;
#[macro_use]
extern crate serde_derive;

mod format;
pub mod kernel;
pub mod process;
pub mod serializer;

use core::fmt::Display;
use serde::{Deserialize, Serialize};

/// Each node has a unique ID that can be used to identify it. The raw value can be accessed within
/// the kernel.
#[cfg(feature = "kernel")]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeId(pub u16);

/// Each node has a unique ID that can be used to identify it.
#[cfg(not(feature = "kernel"))]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeId(pub u16); // TODO: shouldn't be pub

pub const MAX_PROCESSES: usize = u16::max_value() as usize;

#[repr(C, packed)]
pub struct MessageHeader {
    pub destination: NodeId,
    pub payload_length: u16,
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

/// This is implemented by types that can be passed between nodes as messages. It must be encodable
/// as a series of bytes that is independent from the context in which it was produced (no raw
/// pointers or references)
// TODO: should we use SerializeOwned or Serialize<'de> where 'de is the lifetime of the data
// within the raw message? How long do messages stay in the receive-buffer (until they're dropped,
// maybe (how can we implement a custom `drop` on all messages tho? (custom derive could do it)))
pub trait Message<'de>: Serialize + Deserialize<'de> {}

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

    fn read_u16(&self) -> Result<u16> {
        Ok(self.read_u8()? as u16 + self.read_u8()? as u16)
    }

    fn read_u32(&self) -> Result<u32> {
        Ok(self.read_u16()? as u32 + self.read_u16()? as u32)
    }

    fn read_u64(&self) -> Result<u64> {
        Ok(self.read_u32()? as u64 + self.read_u32()? as u64)
    }
}
