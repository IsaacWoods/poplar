use crate::kernel_map;
use acpi::{AcpiHandler, PhysicalMapping};
use bit_field::BitField;
use core::ptr::NonNull;
use hal::memory::PhysicalAddress;
use hal_x86_64::hw::port::Port;
use log::debug;
use pci_types::{ConfigRegionAccess, PciAddress};
use poplar_util::math::align_down;

#[derive(Clone)]
pub struct PoplarAcpiHandler;

impl AcpiHandler for PoplarAcpiHandler {
    unsafe fn map_physical_region<T>(&self, physical_address: usize, size: usize) -> PhysicalMapping<Self, T> {
        let virtual_address = kernel_map::physical_to_virtual(PhysicalAddress::new(physical_address).unwrap());

        PhysicalMapping::new(
            usize::from(physical_address),
            NonNull::new(virtual_address.mut_ptr()).unwrap(),
            size,
            size,
            PoplarAcpiHandler,
        )
    }

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {}
}

pub struct AmlHandler<A>
where
    A: ConfigRegionAccess,
{
    pci_access: A,
}

impl<A> AmlHandler<A>
where
    A: ConfigRegionAccess,
{
    pub fn new(pci_access: A) -> AmlHandler<A> {
        AmlHandler { pci_access }
    }
}

impl<A> aml::Handler for AmlHandler<A>
where
    A: ConfigRegionAccess + Sync,
{
    fn read_u8(&self, address: usize) -> u8 {
        debug!("AML: Reading byte from {:#x}", address);
        let address = hal_x86_64::kernel_map::physical_to_virtual(PhysicalAddress::new(address).unwrap());
        assert!(address.is_aligned(1));
        unsafe { core::ptr::read_volatile(address.ptr()) }
    }

    fn read_u16(&self, address: usize) -> u16 {
        debug!("AML: Reading word from {:#x}", address);
        let address = hal_x86_64::kernel_map::physical_to_virtual(PhysicalAddress::new(address).unwrap());
        assert!(address.is_aligned(2));
        unsafe { core::ptr::read_volatile(address.ptr()) }
    }

    fn read_u32(&self, address: usize) -> u32 {
        debug!("AML: Reading dword from {:#x}", address);
        let address = hal_x86_64::kernel_map::physical_to_virtual(PhysicalAddress::new(address).unwrap());
        assert!(address.is_aligned(4));
        unsafe { core::ptr::read_volatile(address.ptr()) }
    }

    fn read_u64(&self, address: usize) -> u64 {
        debug!("AML: Reading qword from {:#x}", address);
        let address = hal_x86_64::kernel_map::physical_to_virtual(PhysicalAddress::new(address).unwrap());
        assert!(address.is_aligned(8));
        unsafe { core::ptr::read_volatile(address.ptr()) }
    }

    fn write_u8(&mut self, address: usize, value: u8) {
        debug!("AML: Writing byte to {:#x}: {:#x}", address, value);
        let address = hal_x86_64::kernel_map::physical_to_virtual(PhysicalAddress::new(address).unwrap());
        assert!(address.is_aligned(1));
        unsafe { core::ptr::write_volatile(address.mut_ptr(), value) }
    }

    fn write_u16(&mut self, address: usize, value: u16) {
        debug!("AML: Writing word to {:#x}: {:#x}", address, value);
        let address = hal_x86_64::kernel_map::physical_to_virtual(PhysicalAddress::new(address).unwrap());
        assert!(address.is_aligned(2));
        unsafe { core::ptr::write_volatile(address.mut_ptr(), value) }
    }

    fn write_u32(&mut self, address: usize, value: u32) {
        debug!("AML: Writing dword to {:#x}: {:#x}", address, value);
        let address = hal_x86_64::kernel_map::physical_to_virtual(PhysicalAddress::new(address).unwrap());
        assert!(address.is_aligned(4));
        unsafe { core::ptr::write_volatile(address.mut_ptr(), value) }
    }

    fn write_u64(&mut self, address: usize, value: u64) {
        debug!("AML: Writing qword to {:#x}: {:#x}", address, value);
        let address = hal_x86_64::kernel_map::physical_to_virtual(PhysicalAddress::new(address).unwrap());
        assert!(address.is_aligned(8));
        unsafe { core::ptr::write_volatile(address.mut_ptr(), value) }
    }

    fn read_io_u8(&self, port: u16) -> u8 {
        debug!("AML: Reading IO byte from port {:#x}", port);
        unsafe { Port::new(port).read() }
    }

    fn read_io_u16(&self, port: u16) -> u16 {
        debug!("AML: Reading IO word from port {:#x}", port);
        unsafe { Port::new(port).read() }
    }

    fn read_io_u32(&self, port: u16) -> u32 {
        debug!("AML: Reading IO dword from port {:#x}", port);
        unsafe { Port::new(port).read() }
    }

    fn write_io_u8(&self, port: u16, value: u8) {
        debug!("AML: Writing IO byte to port {:#x}: {:#x}", port, value);
        unsafe { Port::new(port).write(value) }
    }

    fn write_io_u16(&self, port: u16, value: u16) {
        debug!("AML: Writing IO word to port {:#x}: {:#x}", port, value);
        unsafe { Port::new(port).write(value) }
    }

    fn write_io_u32(&self, port: u16, value: u32) {
        debug!("AML: Writing IO dword to port {:#x}: {:#x}", port, value);
        unsafe { Port::new(port).write(value) }
    }

    fn read_pci_u8(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16) -> u8 {
        debug!("AML: Reading byte from PCI config space (segment={:#x},bus={:#x},device={:#x},function={:#x},offset={:#x})", segment, bus, device, function, offset);
        let dword_read = unsafe {
            self.pci_access.read(PciAddress::new(segment, bus, device, function), align_down(offset, 0x20))
        };
        let start_bit = (offset % 0x20) as usize;
        dword_read.get_bits(start_bit..(start_bit + 8)) as u8
    }

    fn read_pci_u16(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16) -> u16 {
        debug!("AML: Reading word from PCI config space (segment={:#x},bus={:#x},device={:#x},function={:#x},offset={:#x})", segment, bus, device, function, offset);
        let dword_read = unsafe {
            self.pci_access.read(PciAddress::new(segment, bus, device, function), align_down(offset, 0x20))
        };
        let start_bit = (offset % 0x20) as usize;
        dword_read.get_bits(start_bit..(start_bit + 16)) as u16
    }

    fn read_pci_u32(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16) -> u32 {
        debug!("AML: Reading dword from PCI config space (segment={:#x},bus={:#x},device={:#x},function={:#x},offset={:#x})", segment, bus, device, function, offset);
        unsafe { self.pci_access.read(PciAddress::new(segment, bus, device, function), offset) }
    }

    fn write_pci_u8(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16, value: u8) {
        debug!("AML: Writing byte to PCI config space (segment={:#x},bus={:#x},device={:#x},function={:#x},offset={:#x}): {:#x}", segment, bus, device, function, offset, value);
        unimplemented!()
    }

    fn write_pci_u16(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16, value: u16) {
        debug!("AML: Writing word to PCI config space (segment={:#x},bus={:#x},device={:#x},function={:#x},offset={:#x}): {:#x}", segment, bus, device, function, offset, value);
        unimplemented!()
    }

    fn write_pci_u32(&self, segment: u16, bus: u8, device: u8, function: u8, offset: u16, value: u32) {
        debug!("AML: Writing dword to PCI config space (segment={:#x},bus={:#x},device={:#x},function={:#x},offset={:#x}): {:#x}", segment, bus, device, function, offset, value);
        unsafe { self.pci_access.write(PciAddress::new(segment, bus, device, function), offset, value) }
    }
}
