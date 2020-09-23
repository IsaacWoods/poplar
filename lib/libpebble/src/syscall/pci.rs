use super::{raw, SYSCALL_PCI_GET_INFO};
use bit_field::BitField;
use core::convert::TryFrom;

/// PCIe supports 65536 buses, each with 32 slots, each with 8 possible functions. We cram this into a `u32`:
///
/// 32                              16               8         3      0
///  +-------------------------------+---------------+---------+------+
///  |            segment            |      bus      | device  | func |
///  +-------------------------------+---------------+---------+------+
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
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

#[derive(Clone, Debug, Default)]
#[repr(C)]
pub struct PciDeviceInfo {
    pub address: PciAddress,
    pub vendor_id: u16,
    pub device_id: u16,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PciGetInfoError {
    TaskDoesNotHaveCorrectCapability,
    BufferPointerInvalid,
    BufferNotLargeEnough(u32),
}

// TODO: it would be cool if we could do this with the define_error_type macro
impl TryFrom<usize> for PciGetInfoError {
    type Error = ();

    fn try_from(status: usize) -> Result<Self, Self::Error> {
        match status.get_bits(0..16) {
            1 => Ok(Self::TaskDoesNotHaveCorrectCapability),
            2 => Ok(Self::BufferPointerInvalid),
            3 => Ok(Self::BufferNotLargeEnough(status.get_bits(16..48) as u32)),
            _ => Err(()),
        }
    }
}

impl Into<usize> for PciGetInfoError {
    fn into(self) -> usize {
        match self {
            Self::TaskDoesNotHaveCorrectCapability => 1,
            Self::BufferPointerInvalid => 2,
            Self::BufferNotLargeEnough(num_needed) => {
                let mut result = 3;
                result.set_bits(16..48, num_needed as usize);
                result
            }
        }
    }
}

/// Makes a raw `pci_get_info` system call, given a pointer to a buffer and the size of the buffer. On success,
/// returns the number of entries written into the buffer. For a nicer interface to this system call, see
/// [`pci_get_info_slice`] or [`pci_get_info_vec`].
pub fn pci_get_info(buffer_ptr: *mut PciDeviceInfo, buffer_size: usize) -> Result<usize, PciGetInfoError> {
    let result = unsafe { raw::syscall2(SYSCALL_PCI_GET_INFO, buffer_ptr as usize, buffer_size) };

    if result.get_bits(0..16) == 0 {
        Ok(result.get_bits(16..48))
    } else {
        Err(PciGetInfoError::try_from(result).unwrap())
    }
}
