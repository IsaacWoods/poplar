use crate::{ser::Result, Serialize, Serializer, Writer};

impl<T, const N: usize> Serialize for heapless::Vec<T, N>
where
    T: Serialize,
{
    fn serialize<W>(&self, serializer: &mut Serializer<W>) -> Result<()>
    where
        W: Writer,
    {
        let mut seq = serializer.serialize_seq(self.len() as u32)?;
        for element in self {
            seq.serialize_element(element)?;
        }
        Ok(())
    }
}

impl<const N: usize> Serialize for heapless::String<N> {
    fn serialize<W>(&self, serializer: &mut Serializer<W>) -> Result<()>
    where
        W: Writer,
    {
        serializer.serialize_str(self)
    }
}
