use super::FRAME_SIZE;
use crate::memory::PhysicalAddress;

#[derive(Clone, Copy, Debug)]
pub struct Frame {
    number: u64,
}

impl Frame {
    pub fn contains(address: PhysicalAddress) -> Frame {
        Frame {
            number: u64::from(address) / FRAME_SIZE,
        }
    }

    pub fn start_address(&self) -> PhysicalAddress {
        PhysicalAddress::new(self.number * FRAME_SIZE).unwrap()
    }
}
