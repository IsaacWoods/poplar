mod impls;

#[cfg(feature = "heapless")]
mod heapless;

use crate::Handle;
use core::{convert::TryInto, str};

/// Errors that can occur during deserialization.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    EndOfStream,
    TrailingBytes,
    InvalidHandleSlot(crate::HandleSlot),

    InvalidChar,
    InvalidUtf8,
    InvalidBoolMarker(u8),
    InvalidOptionMarker(u8),
    InvalidEnumTag(u32),
}

pub type Result<T> = core::result::Result<T, Error>;

pub trait Deserialize<'de>: Sized {
    fn deserialize(deserializer: &mut Deserializer<'de>) -> Result<Self>;
}

/// A type implements `DeserializeOwned` if it does not borrow any data out of the buffer. In other words, it can
/// be deserialized for any buffer lifetime.
pub trait DeserializeOwned: for<'de> Deserialize<'de> {}
impl<T> DeserializeOwned for T where T: for<'de> Deserialize<'de> {}

pub struct Deserializer<'de> {
    pub(crate) bytes: &'de [u8],
    pub(crate) handles: &'de [Handle],
}

impl<'de> Deserializer<'de> {
    pub fn from_wire(bytes: &'de [u8], handles: &'de [Handle]) -> Self {
        Deserializer { bytes, handles }
    }

    pub fn deserialize_bool(&mut self) -> Result<bool> {
        match self.take_byte()? {
            crate::MARKER_TRUE => Ok(true),
            crate::MARKER_FALSE => Ok(false),
            tag => Err(Error::InvalidBoolMarker(tag)),
        }
    }

    pub fn deserialize_u8(&mut self) -> Result<u8> {
        self.take_byte()
    }

    pub fn deserialize_u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take::<2>()?))
    }

    pub fn deserialize_u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take::<4>()?))
    }

    pub fn deserialize_u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take::<8>()?))
    }

    pub fn deserialize_u128(&mut self) -> Result<u128> {
        Ok(u128::from_le_bytes(self.take::<16>()?))
    }

    pub fn deserialize_i8(&mut self) -> Result<i8> {
        Ok(self.take_byte()? as i8)
    }

    pub fn deserialize_i16(&mut self) -> Result<i16> {
        Ok(i16::from_le_bytes(self.take::<2>()?))
    }

    pub fn deserialize_i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.take::<4>()?))
    }

    pub fn deserialize_i64(&mut self) -> Result<i64> {
        Ok(i64::from_le_bytes(self.take::<8>()?))
    }

    pub fn deserialize_i128(&mut self) -> Result<i128> {
        Ok(i128::from_le_bytes(self.take::<16>()?))
    }

    pub fn deserialize_f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.take::<4>()?))
    }

    pub fn deserialize_f64(&mut self) -> Result<f64> {
        Ok(f64::from_le_bytes(self.take::<8>()?))
    }

    pub fn deserialize_char(&mut self) -> Result<char> {
        char::from_u32(u32::from_le_bytes(self.take::<4>()?)).ok_or(Error::InvalidChar)
    }

    pub fn deserialize_str(&mut self) -> Result<&'de str> {
        let length = self.deserialize_u32()?;
        let bytes = self.take_n(length as usize)?;
        str::from_utf8(bytes).map_err(|_| Error::InvalidUtf8)
    }

    pub fn deserialize_option<T>(&mut self) -> Result<Option<T>>
    where
        T: ?Sized + Deserialize<'de>,
    {
        let tag = self.take_byte()?;
        match tag {
            crate::MARKER_NONE => Ok(None),
            crate::MARKER_SOME => Ok(Some(T::deserialize(self)?)),
            _ => Err(Error::InvalidOptionMarker(tag)),
        }
    }

    /// Start deserializing an `enum`. Any data contained should be deserialized next.
    pub fn deserialize_enum_tag(&mut self) -> Result<u32> {
        self.deserialize_u32()
    }

    /// Start deserializing a `seq`. Returns the number of elements the caller should deserialize.
    pub fn deserialize_seq_length(&mut self) -> Result<u32> {
        self.deserialize_u32()
    }

    /// Start deserializing a `map`. Returns the number of elements (key-value pairs) the caller should deserialize.
    pub fn deserialize_map_length(&mut self) -> Result<u32> {
        self.deserialize_u32()
    }

    pub fn deserialize_handle(&mut self) -> Result<crate::Handle> {
        let slot = self.deserialize_u8()?;
        match self.handles.get(crate::index_from_handle_slot(slot) as usize) {
            Some(&handle) => Ok(handle),
            None => Err(Error::InvalidHandleSlot(slot)),
        }
    }

    fn take_byte(&mut self) -> Result<u8> {
        let &byte = self.bytes.iter().next().ok_or(Error::EndOfStream)?;
        self.bytes = &self.bytes[1..];
        Ok(byte)
    }

    fn take_n(&mut self, n: usize) -> Result<&'de [u8]> {
        if self.bytes.len() < n {
            return Err(Error::EndOfStream);
        }

        let bytes = &self.bytes[0..n];
        self.bytes = &self.bytes[n..];
        Ok(bytes)
    }

    fn take<const N: usize>(&mut self) -> Result<[u8; N]> {
        if self.bytes.len() < N {
            return Err(Error::EndOfStream);
        }

        let bytes = &self.bytes[0..N];
        self.bytes = &self.bytes[N..];
        Ok(bytes.try_into().unwrap())
    }
}
