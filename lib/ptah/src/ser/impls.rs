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
        for element in self {
            element.serialize(serializer)?;
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

#[cfg(feature = "alloc")]
impl<T> Serialize for alloc::vec::Vec<T>
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

macro impl_for_tuple(($($typ:ident),+) => ($($index:tt),+)) {
    impl<$($typ),+> Serialize for ($($typ,)+)
    where
        $(
            $typ: Serialize
         ),+
    {
        fn serialize<W>(&self, serializer: &mut Serializer<W>) -> Result<()>
        where
            W: Writer,
        {
            $(
                self.$index.serialize(serializer)?;
             )+
            Ok(())
        }
    }
}

impl_for_tuple!((T0) => (0));
impl_for_tuple!((T0, T1) => (0, 1));
impl_for_tuple!((T0, T1, T2) => (0, 1, 2));
impl_for_tuple!((T0, T1, T2, T3) => (0, 1, 2, 3));
impl_for_tuple!((T0, T1, T2, T3, T4) => (0, 1, 2, 3, 4));
impl_for_tuple!((T0, T1, T2, T3, T4, T5) => (0, 1, 2, 3, 4, 5));
impl_for_tuple!((T0, T1, T2, T3, T4, T5, T6) => (0, 1, 2, 3, 4, 5, 6));
impl_for_tuple!((T0, T1, T2, T3, T4, T5, T6, T7) => (0, 1, 2, 3, 4, 5, 6, 7));
impl_for_tuple!((T0, T1, T2, T3, T4, T5, T6, T7, T8) => (0, 1, 2, 3, 4, 5, 6, 7, 8));
impl_for_tuple!((T0, T1, T2, T3, T4, T5, T6, T7, T8, T9) => (0, 1, 2, 3, 4, 5, 6, 7, 8, 9));
impl_for_tuple!((T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10) => (0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10));
impl_for_tuple!((T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11) => (0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11));
impl_for_tuple!((T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12) => (0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12));
impl_for_tuple!((T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13) => (0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13));
impl_for_tuple!((T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14) => (0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14));
impl_for_tuple!((T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15) => (0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15));

#[cfg(feature = "alloc")]
impl<K, V> Serialize for alloc::collections::BTreeMap<K, V>
where
    K: Serialize,
    V: Serialize,
{
    fn serialize<W>(&self, serializer: &mut Serializer<W>) -> Result<()>
    where
        W: Writer,
    {
        let mut map = serializer.serialize_map(self.len() as u32)?;
        for (key, value) in self.iter() {
            map.serialize_key(key)?;
            map.serialize_value(value)?;
        }
        Ok(())
    }
}
