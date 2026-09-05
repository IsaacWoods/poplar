use crate::from_bits::FromBits;

pub const trait Packable: Clone + Copy {
    const SIZE_BITS: u32;

    /// Returns a mask with the first `n` bits set, shifted left by `shift`
    fn mask(n: u32, shift: u32) -> Self;

    /// Calculate, for a given mask, the shift that should be applied to start packing
    /// bits directly *after* the bits that will be set by the mask
    fn shift_for_next(mask: Self) -> u32;

    /// Returns the number of bits required to pack a value, given a mask and shift
    fn num_bits(mask: Self, shift: u32) -> u32;

    /// Given a `mask` and `shift`, returns the maximum value of the `Packable`.
    fn max_value(mask: Self, shift: u32) -> Self;

    /// Given mask and shift values, pack the least-significant [`Self::num_bits()`] bits of `value`
    /// into `self`, returning the result.
    fn pack_truncating(&self, value: Self, mask: Self, shift: u32) -> Self;

    /// Unpack the bits for a given `Packer`, given its `mask` and `shift`.
    fn unpack_bits(value: Self, mask: Self, shift: u32) -> Self;
}

macro_rules! make_packable {
    { $($ty:ident),* } => {
        $(
            const impl Packable for $ty {
                const SIZE_BITS: u32 = $ty::MAX.leading_ones();

                fn mask(n: u32, shift: u32) -> Self {
                    if n == 0 {
                        return 0;
                    }

                    let one: $ty = 1;
                    let msb = one.wrapping_shl(n - 1);
                    let mask = msb | (msb.saturating_sub(1));
                    mask << shift
                }

                fn shift_for_next(mask: Self) -> u32 {
                    Self::SIZE_BITS - mask.leading_zeros()
                }

                fn num_bits(mask: Self, shift: u32) -> u32 {
                    Self::SIZE_BITS - (mask >> shift).leading_zeros()
                }

                fn max_value(mask: Self, shift: u32) -> Self {
                    (1 << Self::num_bits(mask, shift)) - 1
                }

                fn pack_truncating(&self, value: Self, mask: Self, shift: u32) -> Self {
                    let max_value = (1 << Self::num_bits(mask, shift)) - 1;
                    let value = value & max_value;
                    let rest = self & !mask; // leave other bits of `base` alone
                    rest | (value << shift)
                }

                fn unpack_bits(value: Self, mask: Self, shift: u32) -> Self {
                    (value & mask) >> shift
                }
            }
        )*
    }
}

make_packable! { u8, u16, u32, u64, u128, usize }

/// `Packer` represents packing a value into some bits of an underlying value of type `B`.
#[derive(Clone, Copy)]
pub struct Packer<B> {
    mask: B,
    shift: u32,
}

impl<B> Packer<B>
where
    B: const Packable + const Ord,
{
    /// Returns a `Packer` that sets the `n` least-significant bits of the value
    pub const fn least_significant(n: u32) -> Self {
        Self {
            mask: B::mask(n, 0),
            shift: 0,
        }
    }

    /// Returns a new `Packer` that sets the *next* `n` least-significant bits of the value
    pub const fn next(&self, n: u32) -> Self {
        let shift = B::shift_for_next(self.mask);
        let mask = B::mask(n, shift);
        Packer { mask, shift }
    }

    /// Returns a new `Packer` that sets the *next* `T::BITS` least-significant bits of the value
    pub const fn then<T>(&self) -> Packer<B>
    where
        T: const FromBits<B>,
    {
        let packer = self.next(T::BITS);
        assert!(T::BITS >= self.num_bits());
        Packer {
            mask: packer.mask,
            shift: packer.shift,
        }
    }

    /// Returns the number of bits required to pack this value
    pub const fn num_bits(&self) -> u32 {
        B::num_bits(self.mask, self.shift)
    }

    /// Pack the [`self.num_bits()`] least-significant bits from `value` into `base`
    pub const fn pack_truncating<T>(&self, base: B, value: T) -> B
    where
        T: const FromBits<B>,
    {
        base.pack_truncating(value.into_bits(), self.mask, self.shift)
    }

    pub const fn pack<T>(&self, base: B, value: T) -> B
    where
        T: const FromBits<B>,
    {
        let value = value.into_bits();
        // TODO: if `const Display` becomes possible, include values here
        assert!(
            value <= B::max_value(self.mask, self.shift),
            "tried to pack invalid bit pattern into field!",
        );
        base.pack_truncating(value, self.mask, self.shift)
    }

    // TODO: would be nice if this could be const
    pub fn unpack<T>(&self, base: B) -> T
    where
        T: FromBits<B>,
    {
        let bits = B::unpack_bits(base, self.mask, self.shift);
        match T::try_from_bits(bits) {
            Ok(value) => value,
            Err(err) => panic!(
                "Failed to unpack bits from value into a {}: {}",
                core::any::type_name::<T>(),
                err
            ),
        }
    }
}

#[cfg(test)]
mod test {
    use super::Packer;

    #[test]
    fn basic() {
        const FIRST_BYTE: Packer<u32> = Packer::least_significant(8);
        const NEXT_BYTE: Packer<u32> = FIRST_BYTE.next(8);
        const FINAL_2BYTES: Packer<u32> = NEXT_BYTE.then::<u16>();

        assert!(FIRST_BYTE.mask == 0xff && FIRST_BYTE.shift == 0);
        assert!(NEXT_BYTE.mask == 0xff00 && NEXT_BYTE.shift == 8);
        assert!(FINAL_2BYTES.mask == 0xffff0000 && FINAL_2BYTES.shift == 16);

        let foo = 0xc0ffee;
        let foo2 = FIRST_BYTE.pack_truncating(foo, 0);
        assert_eq!(foo2, 0xc0ff00);
        let foo3 = NEXT_BYTE.pack_truncating(foo2, 0x4d);
        assert_eq!(foo3, 0xc04d00);
        let foo4 = FINAL_2BYTES.pack_truncating(foo3, 0xff3c);
        assert_eq!(foo4, 0xff3c4d00);
    }
}
