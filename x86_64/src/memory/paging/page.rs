use super::PAGE_SIZE;
use bit_field::BitField;
use crate::memory::VirtualAddress;

#[derive(Clone, Copy, Debug)]
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
