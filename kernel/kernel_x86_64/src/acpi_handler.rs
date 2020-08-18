use crate::kernel_map;
use acpi::{AcpiHandler, PhysicalMapping};
use core::ptr::NonNull;
use hal::memory::PhysicalAddress;

pub struct PebbleAcpiHandler;

impl AcpiHandler for PebbleAcpiHandler {
    unsafe fn map_physical_region<T>(&mut self, physical_address: usize, size: usize) -> PhysicalMapping<T> {
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

pub struct AmlHandler;

impl aml::Handler for AmlHandler {
    fn read_u8(&self, address: usize) -> u8 {
        let address = hal_x86_64::kernel_map::physical_to_virtual(PhysicalAddress::new(address).unwrap());
        assert!(address.is_aligned(1));
        unsafe { core::ptr::read_volatile(address.ptr()) }
    }

    fn read_u16(&self, address: usize) -> u16 {
        let address = hal_x86_64::kernel_map::physical_to_virtual(PhysicalAddress::new(address).unwrap());
        assert!(address.is_aligned(2));
        unsafe { core::ptr::read_volatile(address.ptr()) }
    }

    fn read_u32(&self, address: usize) -> u32 {
        let address = hal_x86_64::kernel_map::physical_to_virtual(PhysicalAddress::new(address).unwrap());
        assert!(address.is_aligned(4));
        unsafe { core::ptr::read_volatile(address.ptr()) }
    }

    fn read_u64(&self, address: usize) -> u64 {
        let address = hal_x86_64::kernel_map::physical_to_virtual(PhysicalAddress::new(address).unwrap());
        assert!(address.is_aligned(8));
        unsafe { core::ptr::read_volatile(address.ptr()) }
    }

    fn write_u8(&mut self, address: usize, value: u8) {
        unimplemented!()
    }

    fn write_u16(&mut self, address: usize, value: u16) {
        unimplemented!()
    }

    fn write_u32(&mut self, address: usize, value: u32) {
        unimplemented!()
    }

    fn write_u64(&mut self, address: usize, value: u64) {
        unimplemented!()
    }

    fn read_io_u8(&self, port: u16) -> u8 {
        unimplemented!()
    }

    fn read_io_u16(&self, port: u16) -> u16 {
        unimplemented!()
    }

    fn read_io_u32(&self, port: u16) -> u32 {
        unimplemented!()
    }

    fn write_io_u8(&self, port: u16, value: u8) {
        unimplemented!()
    }

    fn write_io_u16(&self, port: u16, value: u16) {
        unimplemented!()
    }

    fn write_io_u32(&self, port: u16, value: u32) {
        unimplemented!()
    }

    fn read_pci_u8(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16) -> u8 {
        unimplemented!()
    }

    fn read_pci_u16(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16) -> u16 {
        unimplemented!()
    }

    fn read_pci_u32(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16) -> u32 {
        unimplemented!()
    }

    fn write_pci_u8(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16, value: u8) {
        unimplemented!()
    }

    fn write_pci_u16(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16, value: u16) {
        unimplemented!()
    }

    fn write_pci_u32(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16, value: u32) {
        unimplemented!()
    }
}
