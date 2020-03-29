use crate::kernel_map;
use acpi::{AcpiHandler, PhysicalMapping};
use core::ptr::NonNull;
use hal::memory::PhysicalAddress;

pub struct PebbleAcpiHandler;

impl AcpiHandler for PebbleAcpiHandler {
    fn map_physical_region<T>(&mut self, physical_address: usize, size: usize) -> PhysicalMapping<T> {
        let virtual_address = kernel_map::physical_to_virtual(PhysicalAddress::new(physical_address).unwrap());

        PhysicalMapping {
            physical_start: usize::from(physical_address),
            virtual_start: NonNull::new(virtual_address.mut_ptr()).unwrap(),
            region_length: size,
            mapped_length: size,
        }
    }

    fn unmap_physical_region<T>(&mut self, _region: PhysicalMapping<T>) {}
}
