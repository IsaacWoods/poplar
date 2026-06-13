use core::fmt::Display;

pub const trait FromBits<B>: Sized {
    /// An error produced when an invalid bit pattern is encountered. If all bit patterns in [`B`]
    /// are valid for the type, this should be `!`.
    type Error: Display;

    /// The number of bits required to represent a value of this type.
    const BITS: u32;

    /// Attempt to convert `bits` into a value of this type. May return an error if `bits` is an
    /// invalid bit pattern for this type.
    fn try_from_bits(bits: B) -> Result<Self, Self::Error>;

    /// Convert `self` into a bit representation in [`B`].
    fn into_bits(self) -> B;
}

macro_rules! impl_for_ty {
    { $(impl FromBits<$($U:ty),+> for $T:ty {})+ } => {
        $(
            $(
                const impl FromBits<$U> for $T {
                    type Error = !;
                    const BITS: u32 = <$U>::BITS;

                    fn try_from_bits(value: $U) -> Result<Self, !> {
                        Ok(value as $T)
                    }

                    fn into_bits(self) -> $U {
                        self as $U
                    }
                }
            )+
        )+
    }
}

impl_for_ty! {
    impl FromBits<u8, u16, u32, u64, u128> for u8 {}
    impl FromBits<u16, u32, u64, u128> for u16 {}
    impl FromBits<u32, u64, u128> for u32 {}
    impl FromBits<u64, u128> for u64 {}
    impl FromBits<u128> for u128 {}

    impl FromBits<u8, u16, u32, u64, u128> for i8 {}
    impl FromBits<u16, u32, u64, u128> for i16 {}
    impl FromBits<u32, u64, u128> for i32 {}
    impl FromBits<u64, u128> for i64 {}

    /*
     * `usize` is at least 16 bits wide as Rust does not support 8-bit targets. Impls for larger
     * types have to be restricted depending on pointer width.
     */
    impl FromBits<usize> for u8 {}
    impl FromBits<usize> for u16 {}
    impl FromBits<usize> for i8 {}
    impl FromBits<usize> for i16 {}

    impl FromBits<usize> for usize {}
    impl FromBits<usize> for isize {}
    impl FromBits<u128> for usize {}
}

#[cfg(target_pointer_width = "16")]
impl_for_ty! {
    impl FromBits<u16, u32, u64> for usize {}
    impl FromBits<u16, u32, u64> for isize {}
}
#[cfg(target_pointer_width = "32")]
impl_for_ty! {
    impl FromBits<u32, u64> for usize {}
    impl FromBits<u32, u64> for isize {}

    impl FromBits<usize> for u32 {}
    impl FromBits<usize> for i32 {}
}
#[cfg(target_pointer_width = "64")]
impl_for_ty! {
    impl FromBits<u64> for usize {}
    impl FromBits<u64> for isize {}

    impl FromBits<usize> for u32 {}
    impl FromBits<usize> for i32 {}
    impl FromBits<usize> for u64 {}
    impl FromBits<usize> for i64 {}
}

macro_rules! impl_for_bool {
    { impl FromBits<$($U:ty),+> for bool {} } => {
        $(
            const impl FromBits<$U> for bool {
                type Error = !;
                const BITS: u32 = 1;

                fn try_from_bits(value: $U) -> Result<Self, !> {
                    Ok(if value == 0 { false } else { true })
                }

                fn into_bits(self) -> $U {
                    if self { 1 } else { 0 }
                }
            }
        )+
    }
}

impl_for_bool! {
    impl FromBits<u8, u16, u32, u64, u128, usize> for bool {}
}
