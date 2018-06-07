use core::{mem, ptr::NonNull, any::Any};
use alloc::BTreeMap;
use acpi::AcpiHandler;
use acpi::PhysicalMapping as AcpiPhysicalMapping;
use memory::MemoryController;
use memory::paging::{PhysicalAddress, PhysicalMapping, EntryFlags};

pub struct PebbleAcpiHandler<'a>
{
    memory_controller   : &'a mut MemoryController,
    // mapped_regions      : BTreeMap<PhysicalAddress, PhysicalMapping<Any>>,
}

impl<'a> PebbleAcpiHandler<'a>
{
    pub fn parse_acpi(memory_controller : &mut MemoryController,
                      rsdt_address      : PhysicalAddress,
                      revision          : u8)
    {
        let mut handler = PebbleAcpiHandler
                          {
                              memory_controller,
                              // mapped_regions        : BTreeMap::new(),
                          };

        match acpi::parse_rsdt(&mut handler, revision, usize::from(rsdt_address))
        {
            Ok(()) => { },

            Err(err) =>
            {
                panic!("Failed to do ACPI stuff and things: {:?}", err);
            },
        }
    }
}

impl<'a> AcpiHandler for PebbleAcpiHandler<'a>
{
    fn map_physical_region<T>(&mut self, physical_address: usize, size: usize) -> AcpiPhysicalMapping<T>
    {
        let address = PhysicalAddress::new(physical_address);
        let physical_mapping = self.memory_controller
                                   .kernel_page_table
                                   .map_physical_region::<T>(address,
                                                             address.offset(size as isize),
                                                             EntryFlags::PRESENT,
                                                             &mut self.memory_controller.frame_allocator);

        let acpi_mapping = AcpiPhysicalMapping
                           {
                               physical_start  : physical_address,
                               virtual_start   : NonNull::<T>::new(physical_mapping.ptr).expect("Physical mapping failed"),
                               region_length   : size,
                               mapped_length   : physical_mapping.size,
                           };

        // self.mapped_regions.insert(address, physical_mapping as PhysicalMapping<Any>);
        acpi_mapping
    }

    fn unmap_physical_region<T>(&mut self, region: AcpiPhysicalMapping<T>)
    {
        // FIXME: unmap the region
        // let mapping = self.mapped_regions.remove(region.physical_start);
        // self.memory_controller.kernel_page_table.unmap_physical_region(mapping,
        //                                                                &mut self.memory_controller.frame_allocator);
    }
}
