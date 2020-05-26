use crate::{Error, Result, Writer};
use core::mem;
use serde::{ser, Serialize};

pub struct Serializer<'w, W>
where
    W: Writer,
{
    writer: &'w mut W,
}

pub fn to_wire<'w, T, W>(value: &T, writer: &'w mut W) -> Result<()>
where
    T: Serialize,
    W: Writer,
{
    let mut serializer = Serializer { writer };

    value.serialize(&mut serializer)?;
    Ok(serializer.output)
}

impl<'a, 'w, W> ser::Serializer for &'a mut Serializer<'w, W>
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
        self.write.write(&mem::transmute::<f32, [u8; 4]>(value))
    }

    fn serialize_f64(self, value: f64) -> Result<Self::Ok> {
        self.write.write(&mem::transmute::<f64, [u8; 8]>(value))
    }

    fn serialize_char(self, value: char) -> Result<Self::Ok> {
        let mut buffer = [0u8; 4];
        value.encode_utf8(&mut buffer);
        self.write.write(&buffer)
    }
}
