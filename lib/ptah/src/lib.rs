#![no_std]
#![feature(const_generics, assoc_char_funcs)]

extern crate alloc;

mod de;
mod ser;

pub use de::Deserializer;
pub use ser::Serializer;

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::fmt;
use serde::{Deserialize, Serialize};

/// It can sometimes be useful to know the size of a value in its serialized form (e.g. to reserve space for it in
/// a ring buffer). This calculates the number of bytes taken to serialize some `value` of `T` into Ptah's wire
/// format. Note that this size is for the specific `value`, and may differ between values of `T`.
pub fn serialized_size<T>(value: &T) -> Result<usize>
where
    T: Serialize,
{
    let mut size = 0;
    let mut serializer = Serializer { writer: SizeCalculator { size: &mut size } };

    value.serialize(&mut serializer)?;
    Ok(size)
}

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
    WriterFull,

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

/// A `Writer` represents a consumer of the bytes produced by serializing a message. In cases where you can
/// create a slice to put the bytes in, `CursorWriter` can be used. Custom `Writer`s are useful for more niche
/// uses, such as sending the serialized bytes over a serial port.
pub trait Writer {
    fn write(&mut self, buf: &[u8]) -> Result<()>;
}

/// This is a `Writer` that can be used to serialize a value into a pre-allocated byte buffer.
pub struct CursorWriter<'a> {
    buffer: &'a mut [u8],
    position: usize,
}

impl<'a> CursorWriter<'a> {
    pub fn new(buffer: &'a mut [u8]) -> CursorWriter<'a> {
        CursorWriter { buffer, position: 0 }
    }
}

impl<'a> Writer for CursorWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> Result<()> {
        /*
         * Detect if the write will overflow the buffer.
         */
        if (self.position + buf.len()) > self.buffer.len() {
            return Err(Error::WriterFull);
        }

        self.buffer[self.position..(self.position + buf.len())].copy_from_slice(buf);
        self.position += buf.len();
        Ok(())
    }
}

impl<'a> Writer for &'a mut Vec<u8> {
    fn write(&mut self, buf: &[u8]) -> Result<()> {
        self.extend_from_slice(buf);
        Ok(())
    }
}

/// This is a writer that can be used to calculate the size of a serialized value. It doesn't actually write the
/// serialized bytes anywhere - it simply tracks how are produced. Because the `Serializer` takes the `Writer` by
/// value, this stores a reference back to the size, so it can be accessed after serialization is complete.
struct SizeCalculator<'a> {
    size: &'a mut usize,
}

impl<'a> Writer for SizeCalculator<'a> {
    fn write(&mut self, buf: &[u8]) -> Result<()> {
        *self.size += buf.len();
        Ok(())
    }
}
