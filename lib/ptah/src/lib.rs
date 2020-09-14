#![no_std]
#![feature(const_generics, assoc_char_funcs)]

extern crate alloc;

mod de;
mod ser;

pub use de::Deserializer;
pub use ser::Serializer;

use alloc::string::{String, ToString};
use core::fmt;
use serde::{Deserialize, Serialize};

pub fn to_wire<'w, T, W>(value: &T, writer: W) -> Result<()>
where
    T: Serialize,
    W: Writer,
{
    let mut serializer = Serializer { writer };

    value.serialize(&mut serializer)?;
    Ok(())
}

pub fn from_wire<'a, T>(serialized: &'a [u8]) -> Result<T>
where
    T: Deserialize<'a>,
{
    let mut deserializer = Deserializer::from_wire(serialized);
    let value = T::deserialize(&mut deserializer)?;
    if deserializer.input.is_empty() {
        Ok(value)
    } else {
        Err(Error::TrailingBytes)
    }
}

/*
 * These are constants that are used in the wire format.
 * TODO: if this stuff grows much more, they can probably get their own module
 */
pub(crate) const MARKER_FALSE: u8 = 0x0;
pub(crate) const MARKER_TRUE: u8 = 0x1;
pub(crate) const MARKER_NONE: u8 = 0x0;
pub(crate) const MARKER_SOME: u8 = 0x1;

type Result<T> = core::result::Result<T, Error>;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Error {
    EndOfStream,
    TrailingBytes,

    ExpectedBool,
    ExpectedUtf8Str,
    InvalidOptionMarker(u8),
    InvalidChar,

    DeserializeAnyNotSupported,

    Custom(String),
}

impl serde::ser::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Error::Custom(msg.to_string())
    }
}

impl serde::de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Error::Custom(msg.to_string())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        unimplemented!()
    }
}

// XXX: in the future, we'll be able to implement Writer for a "slice" of a message buffer shared between a task
// and the kernel
pub trait Writer {
    fn write(&mut self, buf: &[u8]) -> Result<()>;
}
