use crate::de::{Deserialize, Deserializer, Result};
use core::{mem::MaybeUninit, ptr};

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

impl<'de> Deserialize<'de> for &'de str {
    fn deserialize(deserializer: &mut Deserializer<'de>) -> Result<&'de str> {
        deserializer.deserialize_str()
    }
}

impl<'de> Deserialize<'de> for alloc::string::String {
    fn deserialize(deserializer: &mut Deserializer<'de>) -> Result<alloc::string::String> {
        use alloc::string::ToString;
        deserializer.deserialize_str().map(|s| s.to_string())
    }
}

impl<'de, T> Deserialize<'de> for Option<T>
where
    T: ?Sized + Deserialize<'de>,
{
    fn deserialize(deserializer: &mut Deserializer<'de>) -> Result<Option<T>> {
        deserializer.deserialize_option()
    }
}

impl<'de, T, const N: usize> Deserialize<'de> for [T; N]
where
    T: Deserialize<'de>,
{
    fn deserialize(deserializer: &mut Deserializer<'de>) -> Result<[T; N]> {
        let mut array: [MaybeUninit<T>; N] = MaybeUninit::uninit_array();
        let start_ptr: *mut T = MaybeUninit::slice_as_mut_ptr(&mut array);

        for i in 0..N {
            unsafe {
                ptr::write(start_ptr.offset(i as isize), T::deserialize(deserializer)?);
            }
        }

        /*
         * TODO: this isn't great. It feels like there should be a function on MaybeUninit to allow us to do this.
         * We can't use `slice_assume_init_ref` because there's no easy way to then turn that slice into an array
         * without constraining T to be `Copy` or at least `Clone`.
         */
        Ok(unsafe { ptr::read(start_ptr as *const [T; N]) })
    }
}

impl<'de> Deserialize<'de> for () {
    fn deserialize(_deserializer: &mut Deserializer<'de>) -> Result<()> {
        Ok(())
    }
}

#[cfg(feature = "alloc")]
impl<'de, T> Deserialize<'de> for alloc::vec::Vec<T>
where
    T: Deserialize<'de>,
{
    fn deserialize(deserializer: &mut Deserializer<'de>) -> Result<alloc::vec::Vec<T>> {
        let length = deserializer.deserialize_seq_length()?;
        let mut vec = alloc::vec::Vec::with_capacity(length as usize);

        for _ in 0..length {
            vec.push(T::deserialize(deserializer)?);
        }

        Ok(vec)
    }
}

macro impl_for_tuple(($($typ:ident),+)) {
    impl<'de, $($typ),+> Deserialize<'de> for ($($typ,)+)
    where
        $(
            $typ: Deserialize<'de>
         ),+
    {
        fn deserialize(deserializer: &mut Deserializer<'de>) -> Result<Self> {
            Ok(($($typ::deserialize(deserializer)?,)+))
        }
    }
}

impl_for_tuple!((T0));
impl_for_tuple!((T0, T1));
impl_for_tuple!((T0, T1, T2));
impl_for_tuple!((T0, T1, T2, T3));
impl_for_tuple!((T0, T1, T2, T3, T4));
impl_for_tuple!((T0, T1, T2, T3, T4, T5));
impl_for_tuple!((T0, T1, T2, T3, T4, T5, T6));
impl_for_tuple!((T0, T1, T2, T3, T4, T5, T6, T7));
impl_for_tuple!((T0, T1, T2, T3, T4, T5, T6, T7, T8));
impl_for_tuple!((T0, T1, T2, T3, T4, T5, T6, T7, T8, T9));
impl_for_tuple!((T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10));
impl_for_tuple!((T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11));
impl_for_tuple!((T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12));
impl_for_tuple!((T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13));
impl_for_tuple!((T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14));
impl_for_tuple!((T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15));

#[cfg(feature = "alloc")]
impl<'de, K, V> Deserialize<'de> for alloc::collections::BTreeMap<K, V>
where
    K: Ord + Deserialize<'de>,
    V: Deserialize<'de>,
{
    fn deserialize(deserializer: &mut Deserializer<'de>) -> Result<alloc::collections::BTreeMap<K, V>> {
        let length = deserializer.deserialize_map_length()?;
        let mut map = alloc::collections::BTreeMap::new();

        for _ in 0..length {
            map.insert(K::deserialize(deserializer)?, V::deserialize(deserializer)?);
        }

        Ok(map)
    }
}
