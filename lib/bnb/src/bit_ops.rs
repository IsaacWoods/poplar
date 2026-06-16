use core::ops::Range;

pub const trait BitOps {
    const BIT_WIDTH: usize;

    fn bit(&self, bit: usize) -> bool;
    fn bits(&self, bits: Range<usize>) -> Self;

    fn set_bit(&mut self, bit: usize, value: bool) -> &mut Self;
    fn set_bits(&mut self, bits: Range<usize>, value: Self) -> &mut Self;
}

macro_rules! impl_bitops {
    ($t:ty) => {
        const impl BitOps for $t {
            const BIT_WIDTH: usize = core::mem::size_of::<Self>() * 8;

            #[inline]
            #[track_caller]
            fn bit(&self, bit: usize) -> bool {
                assert!(bit < Self::BIT_WIDTH);
                (*self & (1 << bit)) != 0
            }

            #[inline]
            #[track_caller]
            fn bits(&self, bits: Range<usize>) -> Self {
                assert!(bits.start < Self::BIT_WIDTH);
                assert!(bits.end <= Self::BIT_WIDTH);
                assert!(bits.start <= bits.end);

                if bits.start == bits.end {
                    0
                } else {
                    (*self << (Self::BIT_WIDTH - bits.end) >> (Self::BIT_WIDTH - bits.end))
                        >> bits.start
                }
            }

            #[inline]
            #[track_caller]
            fn set_bit(&mut self, bit: usize, value: bool) -> &mut Self {
                assert!(bit < Self::BIT_WIDTH);

                if value {
                    *self |= 1 << bit;
                } else {
                    *self &= !(1 << bit);
                }

                self
            }

            #[inline]
            #[track_caller]
            fn set_bits(&mut self, bits: Range<usize>, value: Self) -> &mut Self {
                assert!(bits.start < Self::BIT_WIDTH);
                assert!(bits.end <= Self::BIT_WIDTH);
                assert!(bits.start <= bits.end);
                assert!(
                    bits.start == bits.end && value == 0
                        || (value << (Self::BIT_WIDTH - (bits.end - bits.start))
                            >> (Self::BIT_WIDTH - (bits.end - bits.start)))
                            == value,
                    "value does not fit into specified bit range!"
                );

                if bits.start != bits.end {
                    let mask = !(!0 << (Self::BIT_WIDTH - bits.end)
                        >> (Self::BIT_WIDTH - bits.end)
                        >> bits.start
                        << bits.start);
                    *self = (*self & mask) | (value << bits.start);
                }

                self
            }
        }
    };
}

impl_bitops!(u8);
impl_bitops!(u16);
impl_bitops!(u32);
impl_bitops!(u64);
impl_bitops!(u128);
impl_bitops!(usize);
impl_bitops!(i8);
impl_bitops!(i16);
impl_bitops!(i32);
impl_bitops!(i64);
impl_bitops!(i128);
impl_bitops!(isize);
