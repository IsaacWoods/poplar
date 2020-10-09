use crate::de::{Deserialize, Deserializer, Result};

macro impl_for_primitive {
    ($ty:ty, $method:ident) => {
        impl<'de> Deserialize<'de> for $ty {
            fn deserialize(deserializer: &mut Deserializer<'de>) -> Result<Self> {
                deserializer.$method()
            }
        }
    },

    (needs_cast $ty:ty, $method:ident) => {
        impl<'de> Deserialize<'de> for $ty {
            fn deserialize(deserializer: &mut Deserializer<'de>) -> Result<Self> {
                deserializer.$method().map(|value| value as $ty)
            }
        }
    }
}

impl_for_primitive!(u8, deserialize_u8);
impl_for_primitive!(u16, deserialize_u16);
impl_for_primitive!(u32, deserialize_u32);
impl_for_primitive!(u64, deserialize_u64);
impl_for_primitive!(u128, deserialize_u128);
impl_for_primitive!(needs_cast usize, deserialize_u64);

impl_for_primitive!(i8, deserialize_i8);
impl_for_primitive!(i16, deserialize_i16);
impl_for_primitive!(i32, deserialize_i32);
impl_for_primitive!(i64, deserialize_i64);
impl_for_primitive!(i128, deserialize_i128);
impl_for_primitive!(needs_cast isize, deserialize_i64);

impl_for_primitive!(f32, deserialize_f32);
impl_for_primitive!(f64, deserialize_f64);

impl_for_primitive!(bool, deserialize_bool);
impl_for_primitive!(char, deserialize_char);

impl<'de, T> Deserialize<'de> for Option<T>
where
    T: ?Sized + Deserialize<'de>,
{
    fn deserialize(deserializer: &mut Deserializer<'de>) -> Result<Option<T>> {
        deserializer.deserialize_option()
    }
}
