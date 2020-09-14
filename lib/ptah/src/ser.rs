use crate::{Error, Result, Writer};
use serde::{ser, Serialize};

pub struct Serializer<W>
where
    W: Writer,
{
    pub(crate) writer: W,
}

pub fn to_wire<'w, T, W>(value: &T, writer: &'w mut W) -> Result<()>
where
    T: Serialize,
    W: Writer,
{
    let mut serializer = Serializer { writer };

    value.serialize(&mut serializer)?;
    Ok(())
}

impl<'a, W> ser::Serializer for &'a mut Serializer<W>
where
    W: Writer,
{
    type Ok = ();
    type Error = Error;

    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, value: bool) -> Result<Self::Ok> {
        match value {
            false => self.writer.write(&[0x00]),
            true => self.writer.write(&[0x01]),
        }
    }

    fn serialize_i8(self, value: i8) -> Result<Self::Ok> {
        self.writer.write(&value.to_le_bytes())
    }

    fn serialize_i16(self, value: i16) -> Result<Self::Ok> {
        self.writer.write(&value.to_le_bytes())
    }

    fn serialize_i32(self, value: i32) -> Result<Self::Ok> {
        self.writer.write(&value.to_le_bytes())
    }

    fn serialize_i64(self, value: i64) -> Result<Self::Ok> {
        self.writer.write(&value.to_le_bytes())
    }

    fn serialize_u8(self, value: u8) -> Result<Self::Ok> {
        self.writer.write(&value.to_le_bytes())
    }

    fn serialize_u16(self, value: u16) -> Result<Self::Ok> {
        self.writer.write(&value.to_le_bytes())
    }

    fn serialize_u32(self, value: u32) -> Result<Self::Ok> {
        self.writer.write(&value.to_le_bytes())
    }

    fn serialize_u64(self, value: u64) -> Result<Self::Ok> {
        self.writer.write(&value.to_le_bytes())
    }

    fn serialize_f32(self, value: f32) -> Result<Self::Ok> {
        self.writer.write(&value.to_le_bytes())
    }

    fn serialize_f64(self, value: f64) -> Result<Self::Ok> {
        self.writer.write(&value.to_le_bytes())
    }

    fn serialize_char(self, value: char) -> Result<Self::Ok> {
        self.writer.write(&(value as u32).to_le_bytes())
    }

    fn serialize_str(self, value: &str) -> Result<Self::Ok> {
        self.serialize_bytes(value.as_bytes())
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok> {
        use ser::SerializeSeq;

        let mut seq = self.serialize_seq(Some(value.len()))?;
        for byte in value {
            seq.serialize_element(byte)?;
        }
        seq.end()
    }

    fn serialize_none(self) -> Result<Self::Ok> {
        self.writer.write(&[crate::MARKER_NONE])
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        self.writer.write(&[crate::MARKER_SOME])?;
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok> {
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(self, _name: &'static str, index: u32, _variant: &'static str) -> Result<Self::Ok> {
        self.serialize_u32(index)
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        /*
         * We just treat the newtype as a transparent wrapper.
         */
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        /*
         * We serialize the variant index, and then the data.
         */
        self.serialize_u32(variant_index)?;
        value.serialize(self)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        /*
         * We encode the length as a `u64`, followed by the data. We only support sequences that know their
         * length upfront.
         */
        self.serialize_u64(len.unwrap() as u64)?;
        Ok(self)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        /*
         * We'll already know the length of the tuple when deserializing, so we don't need to include it here.
         */
        Ok(self)
    }

    fn serialize_tuple_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeTupleStruct> {
        Ok(self)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        self.serialize_u32(variant_index)?;
        Ok(self)
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        self.serialize_u64(len.unwrap() as u64)?;
        Ok(self)
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Ok(self)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        self.serialize_u32(variant_index)?;
        Ok(self)
    }
}

impl<'a, W> ser::SerializeSeq for &'a mut Serializer<W>
where
    W: Writer,
{
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok> {
        Ok(())
    }
}

impl<'a, W> ser::SerializeTuple for &'a mut Serializer<W>
where
    W: Writer,
{
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok> {
        Ok(())
    }
}

impl<'a, W> ser::SerializeTupleStruct for &'a mut Serializer<W>
where
    W: Writer,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok> {
        Ok(())
    }
}

impl<'a, W> ser::SerializeTupleVariant for &'a mut Serializer<W>
where
    W: Writer,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok> {
        Ok(())
    }
}

impl<'a, W> ser::SerializeMap for &'a mut Serializer<W>
where
    W: Writer,
{
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        key.serialize(&mut **self)
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok> {
        Ok(())
    }
}

impl<'a, W> ser::SerializeStruct for &'a mut Serializer<W>
where
    W: Writer,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok> {
        Ok(())
    }
}

impl<'a, W> ser::SerializeStructVariant for &'a mut Serializer<W>
where
    W: Writer,
{
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<Self::Ok>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<Self::Ok> {
        Ok(())
    }
}
