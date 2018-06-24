use core::fmt::Display;
use format;
use serde::ser::{
    SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
    SerializeTupleStruct, SerializeTupleVariant,
};
use serde::{Serialize, Serializer};
use {Error, MessageWriter, Result};

pub struct MessageSerializer<'w, W: MessageWriter + 'w> {
    writer: &'w mut W,
}

impl<'w, W> MessageSerializer<'w, W>
where
    W: MessageWriter + 'w,
{
    pub fn new(writer: &'w mut W) -> MessageSerializer<'w, W> {
        MessageSerializer { writer }
    }
}

impl<'a, 'w, W: MessageWriter> Serializer for &'a mut MessageSerializer<'w, W> {
    /*
     * We don't produce an actual type when we serialize a message, as we're writing it directly
     * into the send buffer.
     */
    type Ok = ();
    type Error = Error;

    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_unit(self) -> Result<()> {
        self.writer.write_u8(format::UNIT)
    }

    fn serialize_bool(self, value: bool) -> Result<()> {
        match value {
            true => self.writer.write_u8(format::BOOL_TRUE),
            false => self.writer.write_u8(format::BOOL_FALSE),
        }
    }

    fn serialize_u8(self, value: u8) -> Result<()> {
        self.writer.write_u8(format::U8)?;
        self.writer.write_u8(value)?;
        Ok(())
    }

    fn serialize_u16(self, value: u16) -> Result<()> {
        self.writer.write_u8(format::U16)?;
        self.writer.write_u16(value)?;
        Ok(())
    }

    fn serialize_u32(self, value: u32) -> Result<()> {
        self.writer.write_u8(format::U32)?;
        self.writer.write_u32(value)?;
        Ok(())
    }

    fn serialize_u64(self, value: u64) -> Result<()> {
        self.writer.write_u8(format::U64)?;
        self.writer.write_u64(value)?;
        Ok(())
    }

    fn serialize_i8(self, value: i8) -> Result<()> {
        self.writer.write_u8(format::I8)?;
        self.writer.write_u8(value as u8)?;
        Ok(())
    }

    fn serialize_i16(self, value: i16) -> Result<()> {
        self.writer.write_u8(format::I16)?;
        self.writer.write_u16(value as u16)?;
        Ok(())
    }

    fn serialize_i32(self, value: i32) -> Result<()> {
        self.writer.write_u8(format::I32)?;
        self.writer.write_u32(value as u32)?;
        Ok(())
    }

    fn serialize_i64(self, value: i64) -> Result<()> {
        self.writer.write_u8(format::I64)?;
        self.writer.write_u64(value as u64)?;
        Ok(())
    }

    fn serialize_f32(self, value: f32) -> Result<()> {
        // TODO
        unimplemented!();
    }

    fn serialize_f64(self, value: f64) -> Result<()> {
        // TODO
        unimplemented!();
    }

    fn serialize_char(self, value: char) -> Result<()> {
        // TODO
        unimplemented!();
    }

    fn serialize_str(self, value: &str) -> Result<()> {
        // TODO
        unimplemented!();
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<()> {
        // TODO
        unimplemented!();
    }

    fn serialize_none(self) -> Result<()> {
        self.serialize_unit()
    }

    fn serialize_some<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        // TODO
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
    ) -> Result<()> {
        // TODO: in the future, maybe mark this so we know we're parsing a unit variant
        /*
         * We keep track of the index of the variant, so we know which one to construct, but we
         * don't need to encode any of its data.
         */
        self.serialize_u32(variant_index)
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        /*
         * We treat newtype structs as transparent wrappers and just serialize the inner type.
         */
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.serialize_u32(variant_index)?;
        value.serialize(self)
    }

    fn serialize_seq(self, length: Option<usize>) -> Result<Self::SerializeSeq> {
        // TODO: reject sequences without a length. Encode length as a u64 (bincode does this
        // anyways, then return Ok(self)
        unimplemented!();
        Ok(self)
    }

    fn serialize_tuple(self, length: usize) -> Result<Self::SerializeTuple> {
        self.serialize_seq(Some(length))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        length: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        self.serialize_seq(Some(length))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        length: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        // TODO
        unimplemented!();
        Ok(self)
    }

    fn serialize_map(self, length: Option<usize>) -> Result<Self::SerializeMap> {
        // TODO
        unimplemented!();
        Ok(self)
    }

    fn serialize_struct(self, _name: &'static str, length: usize) -> Result<Self::SerializeStruct> {
        // TODO
        unimplemented!();
        Ok(self)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        length: usize,
    ) -> Result<Self::SerializeStructVariant> {
        // TODO
        unimplemented!();
        Ok(self)
    }

    fn collect_str<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Display,
    {
        // TODO
        unimplemented!();
        Ok(())
    }
}

impl<'a, 'w, W: MessageWriter> SerializeSeq for &'a mut MessageSerializer<'w, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a, 'w, W: MessageWriter> SerializeTuple for &'a mut MessageSerializer<'w, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a, 'w, W: MessageWriter> SerializeTupleStruct for &'a mut MessageSerializer<'w, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a, 'w, W: MessageWriter> SerializeTupleVariant for &'a mut MessageSerializer<'w, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a, 'w, W: MessageWriter> SerializeMap for &'a mut MessageSerializer<'w, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_key<K>(&mut self, key: &K) -> Result<()>
    where
        K: ?Sized + Serialize,
    {
        key.serialize(&mut **self)
    }

    fn serialize_value<V>(&mut self, value: &V) -> Result<()>
    where
        V: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a, 'w, W: MessageWriter> SerializeStruct for &'a mut MessageSerializer<'w, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl<'a, 'w, W: MessageWriter> SerializeStructVariant for &'a mut MessageSerializer<'w, W> {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}
