use super::PAGE_SIZE;
use crate::memory::VirtualAddress;
use bit_field::BitField;
use core::iter::Step;
use core::ops::{Add, AddAssign};

#[derive(Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub struct Page {
    number: u64,
}

impl Page {
    /// Get the page that contains the given virtual address.
    pub fn contains(address: VirtualAddress) -> Page {
        Page {
            number: u64::from(address) / PAGE_SIZE,
        }
    }

    pub fn start_address(&self) -> VirtualAddress {
        VirtualAddress::new(self.number * PAGE_SIZE).unwrap()
    }

    pub fn p4_index(&self) -> u16 {
        self.number.get_bits(27..36) as u16
    }

    pub fn p3_index(&self) -> u16 {
        self.number.get_bits(18..27) as u16
    }

    pub fn p2_index(&self) -> u16 {
        self.number.get_bits(9..18) as u16
    }

    pub fn p1_index(&self) -> u16 {
        self.number.get_bits(0..9) as u16
    }
}

impl Add<u64> for Page {
    type Output = Page;

    fn add(self, offset: u64) -> Self::Output {
        assert!(VirtualAddress::new((self.number + offset) * PAGE_SIZE).is_some());
        Page {
            number: self.number + offset,
        }
    }
}

impl AddAssign<u64> for Page {
    fn add_assign(&mut self, offset: u64) {
        assert!(VirtualAddress::new((self.number + offset) * PAGE_SIZE).is_some());
        self.number += offset;
    }
}

impl Step for Page {
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
        Page {
            number: self.number + 1,
        }
    }

    fn sub_one(&self) -> Self {
        Page {
            number: self.number - 1,
        }
    }

    fn add_usize(&self, n: usize) -> Option<Self> {
        Some(Page {
            number: self.number.checked_add(n as u64)?,
        })
    }
}
