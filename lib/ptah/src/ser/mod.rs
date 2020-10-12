mod impls;

use crate::Writer;

/// Errors that can occur during serialization.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Error {
    WriterFullOfBytes,
    WriterFullOfHandles,
}

pub type Result<T> = core::result::Result<T, Error>;

pub trait Serialize {
    fn serialize<W>(&self, serializer: &mut Serializer<W>) -> Result<()>
    where
        W: Writer;
}

pub struct Serializer<W>
where
    W: Writer,
{
    writer: W,
}

impl<W> Serializer<W>
where
    W: Writer,
{
    pub fn new(writer: W) -> Serializer<W> {
        Serializer { writer }
    }

    pub fn serialize_bool(&mut self, value: bool) -> Result<()> {
        match value {
            false => self.writer.write(&[0x00]),
            true => self.writer.write(&[0x01]),
        }
    }

    pub fn serialize_u8(&mut self, value: u8) -> Result<()> {
        self.writer.write(&value.to_le_bytes())
    }

    pub fn serialize_u16(&mut self, value: u16) -> Result<()> {
        self.writer.write(&value.to_le_bytes())
    }

    pub fn serialize_u32(&mut self, value: u32) -> Result<()> {
        self.writer.write(&value.to_le_bytes())
    }

    pub fn serialize_u64(&mut self, value: u64) -> Result<()> {
        self.writer.write(&value.to_le_bytes())
    }

    pub fn serialize_u128(&mut self, value: u128) -> Result<()> {
        self.writer.write(&value.to_le_bytes())
    }

    pub fn serialize_i8(&mut self, value: i8) -> Result<()> {
        self.writer.write(&value.to_le_bytes())
    }

    pub fn serialize_i16(&mut self, value: i16) -> Result<()> {
        self.writer.write(&value.to_le_bytes())
    }

    pub fn serialize_i32(&mut self, value: i32) -> Result<()> {
        self.writer.write(&value.to_le_bytes())
    }

    pub fn serialize_i64(&mut self, value: i64) -> Result<()> {
        self.writer.write(&value.to_le_bytes())
    }

    pub fn serialize_i128(&mut self, value: i128) -> Result<()> {
        self.writer.write(&value.to_le_bytes())
    }

    pub fn serialize_f32(&mut self, value: f32) -> Result<()> {
        self.writer.write(&value.to_le_bytes())
    }

    pub fn serialize_f64(&mut self, value: f64) -> Result<()> {
        self.writer.write(&value.to_le_bytes())
    }

    pub fn serialize_char(&mut self, value: char) -> Result<()> {
        self.writer.write(&(value as u32).to_le_bytes())
    }

    pub fn serialize_str(&mut self, value: &str) -> Result<()> {
        let bytes = value.as_bytes();
        let mut seq = self.serialize_seq(bytes.len() as u32)?;

        for byte in bytes {
            seq.serialize_element(byte)?;
        }
        Ok(())
    }

    pub fn serialize_none(&mut self) -> Result<()> {
        self.writer.write(&[crate::MARKER_NONE])
    }

    pub fn serialize_some<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.writer.write(&[crate::MARKER_SOME])?;
        value.serialize(self)
    }

    pub fn serialize_tuple<'a>(&'a mut self) -> Result<TupleSerializer<'a, W>> {
        Ok(TupleSerializer(self))
    }

    pub fn serialize_seq<'a>(&'a mut self, length: u32) -> Result<SeqSerializer<'a, W>> {
        self.serialize_u32(length)?;
        Ok(SeqSerializer(self))
    }

    pub fn serialize_struct<'a>(&'a mut self) -> Result<StructSerializer<'a, W>> {
        Ok(StructSerializer(self))
    }

    pub fn serialize_map<'a>(&'a mut self, length: u32) -> Result<MapSerializer<'a, W>> {
        self.serialize_u32(length)?;
        Ok(MapSerializer(self))
    }

    /// Start serializing an enum - this encodes the tag that specifies which variant is being encoded. Data
    /// contained in the variant should be serialized following this.
    pub fn serialize_enum_variant(&mut self, variant_index: u32) -> Result<()> {
        self.serialize_u32(variant_index)
    }
}

pub struct TupleSerializer<'a, W>(&'a mut Serializer<W>)
where
    W: Writer;

impl<'a, W> TupleSerializer<'a, W>
where
    W: Writer,
{
    pub fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self.0)
    }
}

pub struct SeqSerializer<'a, W>(&'a mut Serializer<W>)
where
    W: Writer;

impl<'a, W> SeqSerializer<'a, W>
where
    W: Writer,
{
    pub fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self.0)
    }
}

pub struct StructSerializer<'a, W>(&'a mut Serializer<W>)
where
    W: Writer;

impl<'a, W> StructSerializer<'a, W>
where
    W: Writer,
{
    pub fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self.0)
    }
}

pub struct MapSerializer<'a, W>(&'a mut Serializer<W>)
where
    W: Writer;

impl<'a, W> MapSerializer<'a, W>
where
    W: Writer,
{
    pub fn serialize_key<K>(&mut self, key: &K) -> Result<()>
    where
        K: ?Sized + Serialize,
    {
        key.serialize(self.0)
    }

    pub fn serialize_value<V>(&mut self, value: &V) -> Result<()>
    where
        V: ?Sized + Serialize,
    {
        value.serialize(self.0)
    }
}
