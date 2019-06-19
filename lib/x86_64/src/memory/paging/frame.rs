use super::FRAME_SIZE;
use crate::memory::PhysicalAddress;
use core::{
    iter::Step,
    ops::{Add, AddAssign},
};

#[derive(Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub struct Frame {
    number: usize,
}

impl Frame {
    pub fn contains(address: PhysicalAddress) -> Frame {
        Frame { number: usize::from(address) / FRAME_SIZE }
    }

    pub fn start_address(&self) -> PhysicalAddress {
        PhysicalAddress::new(self.number * FRAME_SIZE).unwrap()
    }
}

impl Add<usize> for Frame {
    type Output = Frame;

    fn add(self, offset: usize) -> Self::Output {
        assert!(PhysicalAddress::new((self.number + offset) * FRAME_SIZE).is_some());
        Frame { number: self.number + offset }
    }
}

impl AddAssign<usize> for Frame {
    fn add_assign(&mut self, offset: usize) {
        assert!(PhysicalAddress::new((self.number + offset) * FRAME_SIZE).is_some());
        self.number += offset;
    }
}

impl Step for Frame {
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        Some(end.number.checked_sub(start.number)? as usize)
    }

    fn replace_one(&mut self) -> Self {
        self.number = 1;
        *self
    }

    fn replace_zero(&mut self) -> Self {
        self.number = 0;
        *self
    }

    fn add_one(&self) -> Self {
        Frame { number: self.number + 1 }
    }

    fn sub_one(&self) -> Self {
        Frame { number: self.number - 1 }
    }

    fn add_usize(&self, n: usize) -> Option<Self> {
        Some(Frame { number: self.number.checked_add(n as usize)? })
    }
}
