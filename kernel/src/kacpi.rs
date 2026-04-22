use crate::bootinfo::BootInfo;
use acpi::{AcpiTables, Handle, PciAddress, PhysicalMapping, aml::AmlError};
use core::ptr::NonNull;
use tracing::info;

pub fn find_tables(boot_info: &BootInfo) -> AcpiTables<Handler> {
    let Some(rsdp_addr) = boot_info.rsdp_addr() else {
        panic!("Cannot find RSDP address! Booting without ACPI is not supported");
    };

    let tables = match unsafe { AcpiTables::from_rsdp(Handler, usize::from(rsdp_addr)) } {
        Ok(tables) => tables,
        Err(err) => panic!("Error parsing ACPI tables: {:?}", err),
    };

    info!("Found {} ACPI tables:", tables.table_headers().count());
    for (addr, table) in tables.table_headers() {
        info!(
            "    {} {:8x} {:4x} {:2x} {:6} {:8} {:2x} {:4} {:8x}",
            table.signature,
            addr,
            table.length(),
            table.revision(),
            table.oem_id().unwrap_or("??????"),
            table.oem_table_id().unwrap_or("????????"),
            table.oem_revision(),
            table.creator_id().unwrap_or("????"),
            table.creator_revision(),
        );
    }

    tables
}

#[derive(Clone)]
pub struct Handler;

impl acpi::Handler for Handler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> PhysicalMapping<Self, T> {
        PhysicalMapping {
            physical_start: physical_address,
            virtual_start: NonNull::new(
                (hal::mem::kernel_map::PHYSICAL_MAPPING_BASE + physical_address).mut_ptr(),
            )
            .unwrap(),
            region_length: size,
            mapped_length: size,
            handler: self.clone(),
        }
    }

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {}

    fn read_u8(&self, _address: usize) -> u8 {
        todo!()
    }

    fn read_u16(&self, _address: usize) -> u16 {
        todo!()
    }

    fn read_u32(&self, _address: usize) -> u32 {
        todo!()
    }

    fn read_u64(&self, _address: usize) -> u64 {
        todo!()
    }

    fn write_u8(&self, _address: usize, _value: u8) {
        todo!()
    }

    fn write_u16(&self, _address: usize, _value: u16) {
        todo!()
    }

    fn write_u32(&self, _address: usize, _value: u32) {
        todo!()
    }

    fn write_u64(&self, _address: usize, _value: u64) {
        todo!()
    }

    fn read_io_u8(&self, _port: u16) -> u8 {
        todo!()
    }

    fn read_io_u16(&self, _port: u16) -> u16 {
        todo!()
    }

    fn read_io_u32(&self, _port: u16) -> u32 {
        todo!()
    }

    fn write_io_u8(&self, _port: u16, _value: u8) {
        todo!()
    }

    fn write_io_u16(&self, _port: u16, _value: u16) {
        todo!()
    }

    fn write_io_u32(&self, _port: u16, _value: u32) {
        todo!()
    }

    fn read_pci_u8(&self, _address: PciAddress, _offset: u16) -> u8 {
        todo!()
    }

    fn read_pci_u16(&self, _address: PciAddress, _offset: u16) -> u16 {
        todo!()
    }

    fn read_pci_u32(&self, _address: PciAddress, _offset: u16) -> u32 {
        todo!()
    }

    fn write_pci_u8(&self, _address: PciAddress, _offset: u16, _value: u8) {
        todo!()
    }

    fn write_pci_u16(&self, _address: PciAddress, _offset: u16, _value: u16) {
        todo!()
    }

    fn write_pci_u32(&self, _address: PciAddress, _offset: u16, _value: u32) {
        todo!()
    }

    fn nanos_since_boot(&self) -> u64 {
        todo!()
    }

    fn stall(&self, _microseconds: u64) {
        todo!()
    }

    fn sleep(&self, _milliseconds: u64) {
        todo!()
    }

    fn create_mutex(&self) -> Handle {
        todo!()
    }

    fn acquire(&self, _mutex: Handle, _timeout: u16) -> Result<(), AmlError> {
        todo!()
    }

    fn release(&self, _mutex: Handle) {
        todo!()
    }
}
