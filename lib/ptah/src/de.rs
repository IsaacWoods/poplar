use crate::{Error, Result};
use alloc::string::ToString;
use serde::{
    de::{self, DeserializeSeed, EnumAccess, IntoDeserializer, MapAccess, SeqAccess, VariantAccess, Visitor},
    Deserialize,
use core::{convert::TryInto, str};
};

pub struct Deserializer<'de> {
    input: &'de [u8],
}

impl<'de> Deserializer<'de> {
    pub fn from_wire(input: &'de [u8]) -> Self {
        Deserializer { input }
    }
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

impl<'de, 'a> de::Deserializer<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    fn deserialize_any<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::DeserializeAnyNotSupported)
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_bool(self.parse_bool()?)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i8(self.take_byte()? as i8)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i16(i16::from_le_bytes([self.take_byte()?, self.take_byte()?]))
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i32(i32::from_le_bytes([
            self.take_byte()?,
            self.take_byte()?,
            self.take_byte()?,
            self.take_byte()?,
        ]))
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i64(i64::from_le_bytes([
            self.take_byte()?,
            self.take_byte()?,
            self.take_byte()?,
            self.take_byte()?,
            self.take_byte()?,
            self.take_byte()?,
            self.take_byte()?,
            self.take_byte()?,
        ]))
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u8(self.take_byte()?)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u16(u16::from_le_bytes(self.take::<2>()?))
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u32(u32::from_le_bytes(self.take::<4>()?))
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_u64(u64::from_le_bytes(self.take::<8>()?))
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_f32(f32::from_le_bytes(self.take::<4>()?))
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_f64(f64::from_le_bytes(self.take::<8>()?))
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_char(char::from_u32(u32::from_le_bytes(self.take::<4>()?)).ok_or(Error::InvalidChar)?)
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let len = u64::from_le_bytes(self.take::<8>()?);
        let bytes = self.take_n(len as usize)?;
        visitor.visit_borrowed_str(str::from_utf8(bytes).map_err(|_| Error::ExpectedUtf8Str)?)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        // TODO: we might be able to combine the logic to deserialize "seq-like things"
        let length = u64::from_le_bytes(self.take::<8>()?);
        let bytes = self.take_n(length as usize)?;
        /*
         * The whole message slice will last as long as the `Deserializer`, and so we call `visit_borrowed_bytes`
         * here, instead of just `visit_bytes`.
         */
        visitor.visit_borrowed_bytes(bytes)
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let length = u64::from_le_bytes(self.take::<8>()?);
        let bytes = self.take_n(length as usize)?;
        visitor.visit_byte_buf(bytes.to_vec())
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let marker = self.take_byte()?;
        match marker {
            crate::MARKER_NONE => visitor.visit_none(),
            crate::MARKER_SOME => visitor.visit_some(self),
            _ => Err(Error::InvalidOptionMarker(marker)),
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        let length = u64::from_le_bytes(self.take::<8>()?);
        self.deserialize_tuple(length as usize, visitor)
    }

    fn deserialize_tuple<V>(self, length: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        struct SeqAccess<'de, 'b> {
            deserializer: &'b mut Deserializer<'de>,
            length: usize,
        }
        impl<'de, 'b> serde::de::SeqAccess<'de> for SeqAccess<'de, 'b> {
            type Error = Error;

            fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
            where
                T: DeserializeSeed<'de>,
            {
                if self.length > 0 {
                    let value = DeserializeSeed::deserialize(seed, &mut *self.deserializer)?;
                    self.length -= 1;
                    Ok(Some(value))
                } else {
                    Ok(None)
                }
            }
        }

        visitor.visit_seq(SeqAccess { deserializer: self, length })
    }

    fn deserialize_tuple_struct<V>(self, _name: &'static str, length: usize, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_tuple(length, visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        self.deserialize_tuple(fields.len(), visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        todo!()
    }

    fn deserialize_identifier<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::Custom("Ptah doesn't support deserializing by identifier".to_string()))
    }

    fn deserialize_ignored_any<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: Visitor<'de>,
    {
        Err(Error::Custom("Ptah doesn't support deserialize_ignored_any".to_string()))
    }
}

impl<'de> Deserializer<'de> {
    fn take_byte(&mut self) -> Result<u8> {
        let &byte = self.input.iter().next().ok_or(Error::EndOfStream)?;
        self.input = &self.input[1..];
        Ok(byte)
    }

    fn take_n(&mut self, n: usize) -> Result<&'de [u8]> {
        if self.input.len() < n {
            return Err(Error::EndOfStream);
        }

        let bytes = &self.input[0..n];
        self.input = &self.input[n..];
        Ok(bytes)
    }

    fn take<const N: usize>(&mut self) -> Result<[u8; N]> {
        if self.input.len() < N {
            return Err(Error::EndOfStream);
        }

        let bytes = &self.input[0..N];
        self.input = &self.input[N..];
        Ok(bytes.try_into().unwrap())
    }

    fn parse_bool(&mut self) -> Result<bool> {
        match self.take_byte()? {
            crate::MARKER_TRUE => Ok(true),
            crate::MARKER_FALSE => Ok(false),
            _ => Err(Error::ExpectedBool),
        }
    }
}
