use crate::{
    ser::{Result, Serialize, Serializer},
    Writer,
};

macro impl_for_primitive {
    ($ty:ty, $method:ident) => {
        impl Serialize for $ty {
            fn serialize<W>(&self, serializer: &mut Serializer<W>) -> Result<()>
            where
                W: Writer,
            {
                serializer.$method(*self)
            }
        }
    },

    ($ty:ty, $method:ident, $cast: ty) => {
        impl Serialize for $ty {
            fn serialize<W>(&self, serializer: &mut Serializer<W>) -> Result<()>
            where
                W: Writer,
            {
                serializer.$method(*self as $cast)
            }
        }
    }
}

impl_for_primitive!(u8, serialize_u8);
impl_for_primitive!(u16, serialize_u16);
impl_for_primitive!(u32, serialize_u32);
impl_for_primitive!(u64, serialize_u64);
impl_for_primitive!(u128, serialize_u128);
impl_for_primitive!(usize, serialize_u64, u64);

impl_for_primitive!(i8, serialize_i8);
impl_for_primitive!(i16, serialize_i16);
impl_for_primitive!(i32, serialize_i32);
impl_for_primitive!(i64, serialize_i64);
impl_for_primitive!(i128, serialize_i128);
impl_for_primitive!(isize, serialize_i64, i64);

impl_for_primitive!(f32, serialize_f32);
impl_for_primitive!(f64, serialize_f64);

impl_for_primitive!(bool, serialize_bool);
impl_for_primitive!(char, serialize_char);

impl Serialize for str {
    fn serialize<W>(&self, serializer: &mut Serializer<W>) -> Result<()>
    where
        W: Writer,
    {
        serializer.serialize_str(self)
    }
}

#[cfg(feature = "alloc")]
impl Serialize for alloc::string::String {
    fn serialize<W>(&self, serializer: &mut Serializer<W>) -> Result<()>
    where
        W: Writer,
    {
        serializer.serialize_str(self)
    }
}

impl<T> Serialize for Option<T>
where
    T: Serialize,
{
    fn serialize<W>(&self, serializer: &mut Serializer<W>) -> Result<()>
    where
        W: Writer,
    {
        match self {
            Some(ref value) => serializer.serialize_some(value),
            None => serializer.serialize_none(),
        }
    }
}

impl<T, const N: usize> Serialize for [T; N]
where
    T: Serialize,
{
    fn serialize<W>(&self, serializer: &mut Serializer<W>) -> Result<()>
    where
        W: Writer,
    {
        let mut tuple = serializer.serialize_tuple()?;
        for element in self {
            tuple.serialize_element(element)?;
        }
        Ok(())
    }
}

impl<T> Serialize for [T]
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

impl Serialize for () {
    fn serialize<W>(&self, _serializer: &mut Serializer<W>) -> Result<()>
    where
        W: Writer,
    {
        Ok(())
    }
}
