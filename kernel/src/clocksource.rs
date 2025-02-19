use core::ops;

pub trait Clocksource {
    fn nanos_since_boot() -> u64;
}

/// `FractionalFreq` allows a fraction to be performed using integer maths of the form `x * (scalar
/// / 2^shift)`. This is useful for doing `ticks -> ns` conversions for timers where you're given a
/// frequency, or have calibrated it against another clock with known frequency.
///
/// This methodology is also used by Linux's clocksource subsystem.
#[derive(Clone, Copy)]
pub struct FractionalFreq {
    scalar: u64,
    shift: u64,
}

const fn ceiling_log2(value: u64) -> u64 {
    64 - value.leading_zeros() as u64
}

impl FractionalFreq {
    pub const fn zero() -> FractionalFreq {
        FractionalFreq { scalar: 0, shift: 0 }
    }

    pub const fn new(numerator: u64, denominator: u64) -> FractionalFreq {
        let shift = 63 - ceiling_log2(numerator);
        let scalar = (numerator << shift) / denominator;
        FractionalFreq { scalar, shift }
    }
}

impl ops::Mul<u64> for FractionalFreq {
    type Output = u64;

    fn mul(self, rhs: u64) -> Self::Output {
        (((rhs as u128) * (self.scalar as u128)) >> self.shift) as u64
    }
}
