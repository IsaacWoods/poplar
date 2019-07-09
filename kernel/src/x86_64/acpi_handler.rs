use acpi::{AcpiHandler, PhysicalMapping as AcpiPhysicalMapping};
use core::ptr::NonNull;
use x86_64::memory::{kernel_map, PhysicalAddress};

pub struct PebbleAcpiHandler;

impl AcpiHandler for PebbleAcpiHandler {
    fn map_physical_region<T>(
        &mut self,
        physical_address: usize,
        size: usize,
    ) -> AcpiPhysicalMapping<T> {
        let virtual_address =
            kernel_map::physical_to_virtual(PhysicalAddress::new(physical_address).unwrap());

        AcpiPhysicalMapping {
            physical_start: usize::from(physical_address),
            virtual_start: NonNull::new(virtual_address.mut_ptr()).unwrap(),
            region_length: size,
            mapped_length: size,
        }
    }

    fn unmap_physical_region<T>(&mut self, _region: AcpiPhysicalMapping<T>) {}
}
