use super::memory::{
    physical::LockedPhysicalMemoryManager, KernelPageTable, PhysicalMapping, PhysicalRegionMapper,
};
use crate::util::math::ceiling_integer_divide;
use acpi::{AcpiHandler, PhysicalMapping as AcpiPhysicalMapping};
use core::ptr::NonNull;
use log::info;
use spin::Mutex;
use x86_64::memory::paging::entry::EntryFlags;
use x86_64::memory::paging::{Frame, FRAME_SIZE};
use x86_64::memory::{PhysicalAddress, VirtualAddress};

pub struct PebbleAcpiHandler<'a> {
    physical_region_mapper: &'a Mutex<PhysicalRegionMapper>,
    page_table: &'a Mutex<KernelPageTable>,
    frame_allocator: &'a LockedPhysicalMemoryManager,
}

impl<'a> PebbleAcpiHandler<'a> {
    pub fn new(
        physical_region_mapper: &'a Mutex<PhysicalRegionMapper>,
        page_table: &'a Mutex<KernelPageTable>,
        frame_allocator: &'a LockedPhysicalMemoryManager,
    ) -> PebbleAcpiHandler<'a> {
        PebbleAcpiHandler {
            physical_region_mapper,
            page_table,
            frame_allocator,
        }
    }
}

impl<'a> AcpiHandler for PebbleAcpiHandler<'a> {
    fn map_physical_region<T>(
        &mut self,
        physical_address: usize,
        size: usize,
    ) -> AcpiPhysicalMapping<T> {
        let mapping = self.physical_region_mapper.lock().map_physical_region(
            Frame::contains(PhysicalAddress::new(physical_address).unwrap()),
            ceiling_integer_divide(size as u64, FRAME_SIZE as u64) as usize,
            EntryFlags::PRESENT | EntryFlags::NO_EXECUTE,
            &mut *self.page_table.lock(),
            self.frame_allocator,
        );

        AcpiPhysicalMapping {
            physical_start: usize::from(mapping.physical_base),
            virtual_start: NonNull::new(usize::from(mapping.virtual_base) as *mut _).unwrap(),
            region_length: size,
            mapped_length: mapping.size,
        }
    }

    fn unmap_physical_region<T>(&mut self, region: AcpiPhysicalMapping<T>) {
        self.physical_region_mapper.lock().unmap_physical_region(
            PhysicalMapping {
                physical_base: PhysicalAddress::new(region.physical_start).unwrap(),
                virtual_base: VirtualAddress::new(region.virtual_start.as_ptr() as usize).unwrap(),
                size: region.mapped_length,
            },
            &mut *self.page_table.lock(),
            self.frame_allocator,
        );
    }
}
