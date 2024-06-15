use crate::{syscall::pci::PciGetInfoError, Handle};
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
    /// A handle to an `Event` that is signalled when this PCI device issues an interrupt.
    pub interrupt: Option<Handle>,
}

#[derive(Debug)]
#[repr(C)]
pub enum Bar {
    Memory32 { memory_object: Handle, size: u32 },
    Memory64 { memory_object: Handle, size: u64 },
}

pub fn pci_get_info_slice(buffer: &mut [PciDeviceInfo]) -> Result<&mut [PciDeviceInfo], PciGetInfoError> {
    match crate::syscall::pci_get_info(
        if buffer.len() == 0 { 0x0 as *mut u8 } else { buffer.as_mut_ptr() as *mut u8 },
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
    let num_descriptors = match crate::syscall::pci_get_info(0x0 as *mut u8, 0) {
        Ok(_) => panic!("pci_get_info with null buffer succeeded."),
        Err(PciGetInfoError::BufferNotLargeEnough(num_descriptors)) => num_descriptors as usize,
        Err(err) => return Err(err),
    };

    // Then actually fetch the data
    let mut descriptors: Vec<PciDeviceInfo> = Vec::with_capacity(num_descriptors);
    assert_eq!(
        crate::syscall::pci_get_info(descriptors.as_mut_ptr() as *mut u8, num_descriptors)?,
        num_descriptors
    );
    unsafe {
        descriptors.set_len(num_descriptors);
    }

    Ok(descriptors)
}
