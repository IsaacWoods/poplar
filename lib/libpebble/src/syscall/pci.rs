use super::{
    raw,
    result::{define_error_type, status_from_syscall_repr},
    SYSCALL_PCI_GET_INFO,
};
use bit_field::BitField;

/// PCIe supports 65536 buses, each with 32 slots, each with 8 possible functions. We cram this into a `u32`:
///
/// 32                              16               8         3      0
///  +-------------------------------+---------------+---------+------+
///  |            segment            |      bus      | device  | func |
///  +-------------------------------+---------------+---------+------+
pub struct PciAddress(u32);

impl PciAddress {
    pub fn new(segment: u16, bus: u8, device: u8, function: u8) -> PciAddress {
        let mut result = 0;
        result.set_bits(0..3, function as u32);
        result.set_bits(3..8, device as u32);
        result.set_bits(8..16, bus as u32);
        result.set_bits(16..32, segment as u32);
        PciAddress(result)
    }

    pub fn segment(&self) -> u16 {
        self.0.get_bits(16..32) as u16
    }

    pub fn bus(&self) -> u8 {
        self.0.get_bits(8..16) as u8
    }

    pub fn device(&self) -> u8 {
        self.0.get_bits(3..8) as u8
    }

    pub fn function(&self) -> u8 {
        self.0.get_bits(0..3) as u8
    }
}

#[repr(C)]
pub struct PciDeviceInfo {
    address: PciAddress,
    vendor_id: u16,
    device_id: u16,
}

define_error_type!(PciGetInfoError {
    TaskDoesNotHaveCorrectCapability => 1,
    BufferPointerInvalid => 2,
    BufferNotLargeEnough => 3,
});

pub fn pci_get_info(buffer: &mut [PciDeviceInfo]) -> Result<&mut [PciDeviceInfo], PciGetInfoError> {
    let result = unsafe { raw::syscall2(SYSCALL_PCI_GET_INFO, buffer.as_ptr() as usize, buffer.len()) };
    status_from_syscall_repr(result.get_bits(0..16))?;

    let valid_length = result.get_bits(16..48);
    Ok(&mut buffer[0..valid_length])
}
