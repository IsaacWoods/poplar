use super::{raw, SYSCALL_PCI_GET_INFO};
use bit_field::BitField;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PciGetInfoError {
    TaskDoesNotHaveCorrectCapability,
    BufferPointerInvalid,
    BufferNotLargeEnough(u32),
    PlatformDoesNotSupportPci,
}

// TODO: it would be cool if we could do this with the define_error_type macro
impl TryFrom<usize> for PciGetInfoError {
    type Error = ();

    fn try_from(status: usize) -> Result<Self, Self::Error> {
        match status.get_bits(0..16) {
            1 => Ok(Self::TaskDoesNotHaveCorrectCapability),
            2 => Ok(Self::BufferPointerInvalid),
            3 => Ok(Self::BufferNotLargeEnough(status.get_bits(16..48) as u32)),
            4 => Ok(Self::PlatformDoesNotSupportPci),
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
            Self::PlatformDoesNotSupportPci => 4,
        }
    }
}

/// Makes a raw `pci_get_info` system call, given a pointer to a buffer and the size of the buffer. On success,
/// returns the number of entries written into the buffer. For a nicer interface to this system call, see
/// [`crate::ddk::pci::pci_get_info_slice`] or [`crate::ddk::pci::pci_get_info_vec`] - these are
/// part of the DDK to avoid pulling the `pci_types` crate into everything that uses this crate.
pub fn pci_get_info(buffer_ptr: *mut u8, buffer_size: usize) -> Result<usize, PciGetInfoError> {
    let result = unsafe { raw::syscall2(SYSCALL_PCI_GET_INFO, buffer_ptr as usize, buffer_size) };

    if result.get_bits(0..16) == 0 {
        Ok(result.get_bits(16..48))
    } else {
        Err(PciGetInfoError::try_from(result).unwrap())
    }
}
