use bit_field::BitField;
use core::mem;
use num::PrimInt;

/// This represents what a type needs to implement to be suitable to back a `Bitmap`.
pub trait BitmapStorage = PrimInt + BitField;

/// `Bitmap` wraps a backing integer type such as `u32` and represents an array of
/// individually-accessible bits.
pub struct Bitmap<T: BitmapStorage>(T);

impl<T> Bitmap<T> where T: BitmapStorage {
    pub fn new(initial: T) -> Bitmap<T> {
        Bitmap(initial)
    }

    pub fn get(&self, bit: usize) -> bool {
        self.0.get_bit(bit)
    }

    pub fn set(&mut self, bit: usize, value: bool) {
        self.0.set_bit(bit, value);
    }

    /// Find `n` consecutive unset bits, set them and return the index of the first bit.
    /// This is useful for memory managers using `Bitmap` to track free frames / pages.
    pub fn alloc_n(&mut self, n: usize) -> Option<T> {
        let num_bits = 8 * mem::size_of::<T>();
        let mask = T::from((T::one() << n) - T::one()).unwrap();

        /*
         * For each bit before there are no longer `n` bits to the end, take the next `n` bits and
         * and them with a mask of `n` ones. If the result is zero, all the bits in the slice must
         * be 0 and so we've found a run of `n` zeros.
         */
        for i in 0..(num_bits - n) {
            if self.0.get_bits(i..(i + n)) & mask == T::zero() {
                self.0.set_bits(i..(i + n), mask);
                return Some(T::from(i).unwrap());
            }
        }

        None
    }
}

#[test]
fn test_bitmap_alloc_n() {
    assert_eq!(Bitmap(0b10001 : u16).alloc_n(3), Some(1));
    assert_eq!(Bitmap(0b11_0000_1_000_111 : u16).alloc_n(4), Some(7));
    assert_eq!(Bitmap(0b1111_1111 : u8).alloc_n(1), None);
}
