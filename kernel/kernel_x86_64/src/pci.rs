use acpi::PciConfigRegions;
use alloc::collections::BTreeMap;
use aml::{pci_routing::PciRoutingTable, AmlContext, AmlName};
use core::ptr;
use hal::{
    memory::PhysicalAddress,
    pci::{ConfigRegionAccess, DeviceId, PciAddress, PciHeader, VendorId},
};
use hal_x86_64::kernel_map;
use log::info;

// TODO: this could probably live in `kernel`
pub struct PciDevice {
    pub vendor_id: VendorId,
    pub device_id: DeviceId,
}

pub struct PciInfo {
    pub devices: BTreeMap<PciAddress, PciDevice>,
}

#[derive(Clone)]
pub struct EcamAccess<'a>(&'a PciConfigRegions);

impl<'a> EcamAccess<'a> {
    pub fn new(regions: &'a PciConfigRegions) -> EcamAccess<'a> {
        EcamAccess(regions)
    }
}

impl<'a> ConfigRegionAccess for EcamAccess<'a> {
    fn function_exists(&self, address: PciAddress) -> bool {
        self.0.physical_address(address.segment, address.bus, address.device, address.function).is_some()
    }

    unsafe fn read(&self, address: PciAddress, offset: u16) -> u32 {
        let physical_address =
            self.0.physical_address(address.segment, address.bus, address.device, address.function).unwrap();
        let ptr = (kernel_map::physical_to_virtual(PhysicalAddress::new(physical_address as usize).unwrap())
            + offset as usize)
            .ptr();
        ptr::read_volatile(ptr)
    }

    unsafe fn write(&self, address: PciAddress, offset: u16, value: u32) {
        let physical_address =
            self.0.physical_address(address.segment, address.bus, address.device, address.function).unwrap();
        let ptr = (kernel_map::physical_to_virtual(PhysicalAddress::new(physical_address as usize).unwrap())
            + offset as usize)
            .mut_ptr();
        ptr::write_volatile(ptr, value)
    }
}

pub struct PciResolver<A>
where
    A: ConfigRegionAccess,
{
    access: A,
    info: PciInfo,
}

impl<A> PciResolver<A>
where
    A: ConfigRegionAccess,
{
    pub fn resolve(access: A) -> PciInfo {
        let mut resolver = Self { access, info: PciInfo { devices: BTreeMap::new() } };

        /*
         * If the device at 0:0:0:0 has multiple functions, there are multiple PCI host controllers, so we need to
         * check all the functions.
         */
        if PciHeader::new(PciAddress { segment: 0, bus: 0, device: 0, function: 0 })
            .has_multiple_functions(&resolver.access)
        {
            for bus in 0..8 {
                resolver.check_bus(bus);
            }
        } else {
            resolver.check_bus(0);
        }

        resolver.info
    }

    fn check_bus(&mut self, bus: u8) {
        for device in 0..32 {
            self.check_device(bus, device);
        }
    }

    fn check_device(&mut self, bus: u8, device: u8) {
        let address = PciAddress { segment: 0, bus, device, function: 0 };
        if self.access.function_exists(address) {
            self.check_function(bus, device, 0);

            let header = PciHeader::new(address);
            if header.has_multiple_functions(&self.access) {
                /*
                 * The device is multi-function. We need to check the rest.
                 */
                for function in 1..8 {
                    self.check_function(bus, device, function);
                }
            }
        }
    }

    fn check_function(&mut self, bus: u8, device: u8, function: u8) {
        let address = PciAddress { segment: 0, bus, device, function };
        if self.access.function_exists(address) {
            let header = PciHeader::new(address);
            let (vendor_id, device_id) = header.id(&self.access);

            if vendor_id == 0xffff {
                return;
            }

            // TODO: check if the function is a PCI-to-PCI bridge, and if so, call check_bus on the secondary bus
            // number (from the bridge's config space)

            info!(
                "Found PCI device (bus={}, device={}, function={}): (vendor = {:#x}, device = {:#x})",
                bus, device, function, vendor_id, device_id
            );

            self.info.devices.insert(address, PciDevice { vendor_id, device_id });
        }
    }
}
