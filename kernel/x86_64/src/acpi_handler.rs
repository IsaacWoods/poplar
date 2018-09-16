use acpi::PhysicalMapping as AcpiPhysicalMapping;
use acpi::{Acpi, AcpiHandler};
use core::ptr::NonNull;
use memory::paging::{EntryFlags, PhysicalAddress};
use memory::MemoryController;

pub struct PebbleAcpiHandler<'a> {
    memory_controller: &'a mut MemoryController,
    // mapped_regions      : BTreeMap<PhysicalAddress, PhysicalMapping<Any>>,
}

impl<'a> PebbleAcpiHandler<'a> {
    pub fn parse_acpi(
        memory_controller: &'a mut MemoryController,
        rsdt_address: PhysicalAddress,
        revision: u8,
    ) -> Result<Acpi, ()> {
        let mut handler = PebbleAcpiHandler {
            memory_controller,
            // mapped_regions        : BTreeMap::new(),
        };

        match acpi::parse_rsdt(&mut handler, revision, usize::from(rsdt_address)) {
            Ok(acpi) => Ok(acpi),

            Err(err) => {
                error!("Failed to parse system's ACPI tables: {:?}", err);
                warn!("Continuing. Some functionality may not work, or the kernel may crash!");
                Err(())
            }
        }
    }
}

impl<'a> AcpiHandler for PebbleAcpiHandler<'a> {
    fn map_physical_region<T>(
        &mut self,
        physical_address: usize,
        size: usize,
    ) -> AcpiPhysicalMapping<T> {
        let address = PhysicalAddress::new(physical_address);
        let physical_mapping = self
            .memory_controller
            .kernel_page_table
            .map_physical_region::<T>(
                address,
                address.offset(size as isize),
                EntryFlags::PRESENT,
                &mut self.memory_controller.frame_allocator,
            );

        let acpi_mapping = AcpiPhysicalMapping {
            physical_start: physical_address,
            virtual_start: NonNull::<T>::new(physical_mapping.ptr)
                .expect("Physical mapping failed"),
            region_length: size,
            mapped_length: physical_mapping.size,
        };

        // self.mapped_regions.insert(address, physical_mapping as PhysicalMapping<Any>);
        acpi_mapping
    }

    fn unmap_physical_region<T>(&mut self, region: AcpiPhysicalMapping<T>) {
        // FIXME: unmap the region
        // let mapping = self.mapped_regions.remove(region.physical_start);
        // self.memory_controller.kernel_page_table.unmap_physical_region(mapping,
        //                                                                &mut self.memory_controller.frame_allocator);
    }
}
