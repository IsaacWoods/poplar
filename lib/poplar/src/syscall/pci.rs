use super::{raw, SYSCALL_PCI_GET_INFO};
use crate::Handle;
use bit_field::BitField;
use pci_types::{BaseClass, DeviceId, DeviceRevision, Interface, PciAddress, SubClass, VendorId};

#[derive(Debug, Default)]
#[repr(C)]
pub struct PciDeviceInfo {
    pub address: PciAddress,
    /// The ID of the manufacturer of the device. These are allocated by PCI SIG.
    pub vendor_id: VendorId,
    /// The ID of the particular device. These are allocated by the vendor.
    pub device_id: DeviceId,
    /// A device-specific revision identifier. These are chosen by the vendor, and should be thought of as a
    /// vendor-defined extension of the device ID.
    pub revision: DeviceRevision,
    /// The upper byte of the class-code. This identifies the Base Class of the device.
    pub class: BaseClass,
    /// The middle byte of the class-code. This identifies the Sub Class of the device.
    pub sub_class: SubClass,
    /// The lower byte of the class-code. This may indicate a specific register-level programming interface of the
    /// device.
    pub interface: Interface,
    pub bars: [Option<Bar>; 6],
}

#[derive(Debug)]
#[repr(C)]
pub enum Bar {
    Memory32 { memory_object: Handle, size: u32 },
    Memory64 { memory_object: Handle, size: u64 },
}

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
/// [`pci_get_info_slice`] or [`pci_get_info_vec`].
pub fn pci_get_info(buffer_ptr: *mut PciDeviceInfo, buffer_size: usize) -> Result<usize, PciGetInfoError> {
    let result = unsafe { raw::syscall2(SYSCALL_PCI_GET_INFO, buffer_ptr as usize, buffer_size) };

    if result.get_bits(0..16) == 0 {
        Ok(result.get_bits(16..48))
    } else {
        Err(PciGetInfoError::try_from(result).unwrap())
    }
}

pub fn pci_get_info_slice(buffer: &mut [PciDeviceInfo]) -> Result<&mut [PciDeviceInfo], PciGetInfoError> {
    match pci_get_info(
        if buffer.len() == 0 { 0x0 as *mut PciDeviceInfo } else { buffer.as_mut_ptr() },
        buffer.len(),
    ) {
        Ok(valid_entries) => Ok(&mut buffer[0..valid_entries]),
        Err(err) => Err(err),
    }
}

#[cfg(feature = "can_alloc")]
pub fn pci_get_info_vec() -> Result<alloc::vec::Vec<PciDeviceInfo>, PciGetInfoError> {
    use alloc::vec::Vec;

    // Make an initial call to find out how many descriptors there are
    let num_descriptors = match pci_get_info(0x0 as *mut PciDeviceInfo, 0) {
        Ok(_) => panic!("pci_get_info with null buffer succeeded."),
        Err(PciGetInfoError::BufferNotLargeEnough(num_descriptors)) => num_descriptors as usize,
        Err(err) => return Err(err),
    };

    // Then actually fetch the data
    let mut descriptors = Vec::with_capacity(num_descriptors);
    assert_eq!(pci_get_info(descriptors.as_mut_ptr(), num_descriptors)?, num_descriptors);
    unsafe {
        descriptors.set_len(num_descriptors);
    }

    Ok(descriptors)
}
