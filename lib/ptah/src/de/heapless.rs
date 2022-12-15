use crate::de::{Deserialize, Deserializer, Result};

impl<'de, T, const N: usize> Deserialize<'de> for heapless::Vec<T, N>
where
    T: Deserialize<'de>,
{
    fn deserialize(deserializer: &mut Deserializer<'de>) -> Result<heapless::Vec<T, N>> {
        let length = deserializer.deserialize_seq_length()?;
        assert!(length as usize <= N);
        let mut vec = heapless::Vec::new();

        for _ in 0..length {
            // TODO: either map error or use `push_unchecked` bc we check it explicitely
            vec.push(T::deserialize(deserializer)?);
        }

        Ok(vec)
    }
}

impl<'de, const N: usize> Deserialize<'de> for heapless::String<N> {
    fn deserialize(deserializer: &mut Deserializer<'de>) -> Result<heapless::String<N>> {
        Ok(heapless::String::from(deserializer.deserialize_str()?))
    }
}
