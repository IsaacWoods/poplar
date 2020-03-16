#![no_std]
#![feature(asm, decl_macro, const_fn)]

pub mod hw;
pub mod kernel_map;
pub mod paging;

use bit_field::BitField;
use hal::memory::VirtualAddress;

pub trait VirtualAddressEx {
    fn p4_index(self) -> usize;
    fn p3_index(self) -> usize;
    fn p2_index(self) -> usize;
    fn p1_index(self) -> usize;
}

impl VirtualAddressEx for VirtualAddress {
    fn p4_index(self) -> usize {
        usize::from(self).get_bits(39..48)
    }

    fn p3_index(self) -> usize {
        usize::from(self).get_bits(30..39)
    }

    fn p2_index(self) -> usize {
        usize::from(self).get_bits(21..30)
    }

    fn p1_index(self) -> usize {
        usize::from(self).get_bits(12..21)
    }
}
