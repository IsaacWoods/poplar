use acpi::PciConfigRegions;
use aml::{pci_routing::PciRoutingTable, AmlContext, AmlName};
use bit_field::BitField;
use core::ptr;
use hal::{
    memory::PhysicalAddress,
    pci::{ConfigRegionAccess, PciAddress, PciHeader},
};
use hal_x86_64::kernel_map;
use log::info;

pub struct PciInfo {}

pub struct PciResolver<'a> {
    pci_info: PciInfo,

    config_regions: &'a PciConfigRegions,
    /*
     * TODO: we currently only support a single root complex (and so only have one routing table). For hardware
     * with multiple root complexes, we would need to keep track of a routing table per root complex.
     */
    routing_table: PciRoutingTable,
}

impl<'a> ConfigRegionAccess for PciResolver<'a> {
    fn function_exists(&self, address: PciAddress) -> bool {
        self.config_regions
            .physical_address(address.segment, address.bus, address.device, address.function)
            .is_some()
    }

    unsafe fn read(&self, address: PciAddress, offset: u16) -> u32 {
        let physical_address = self
            .config_regions
            .physical_address(address.segment, address.bus, address.device, address.function)
            .unwrap();
        let ptr = (kernel_map::physical_to_virtual(PhysicalAddress::new(physical_address as usize).unwrap())
            + offset as usize)
            .ptr();
        unsafe { ptr::read_volatile(ptr) }
    }

    unsafe fn write(&self, address: PciAddress, offset: u16, value: u32) {
        let physical_address = self
            .config_regions
            .physical_address(address.segment, address.bus, address.device, address.function)
            .unwrap();
        let ptr = (kernel_map::physical_to_virtual(PhysicalAddress::new(physical_address as usize).unwrap())
            + offset as usize)
            .mut_ptr();
        unsafe { ptr::write_volatile(ptr, value) }
    }
}

impl<'a> PciResolver<'a> {
    pub fn resolve(config_regions: &'a PciConfigRegions, aml_context: &mut AmlContext) -> PciInfo {
        let routing_table =
            PciRoutingTable::from_prt_path(&AmlName::from_str("\\_SB.PCI0._PRT").unwrap(), aml_context)
                .expect("Failed to parse _PRT");
        let resolver = Self { pci_info: PciInfo {}, config_regions, routing_table };

        /*
         * If the device at 0:0:0:0 has multiple functions, there are multiple PCI host controllers, so we need to
         * check all the functions.
         */
        if PciHeader::new(PciAddress { segment: 0, bus: 0, device: 0, function: 0 })
            .has_multiple_functions(&resolver)
        {
            for bus in 0..8 {
                resolver.check_bus(bus);
            }
        } else {
            resolver.check_bus(0);
        }

        resolver.pci_info
    }

    fn check_bus(&self, bus: u8) {
        for device in 0..32 {
            self.check_device(bus, device);
        }
    }

    fn check_device(&self, bus: u8, device: u8) {
        let address = PciAddress { segment: 0, bus, device, function: 0 };
        if self.function_exists(address) {
            self.check_function(bus, device, 0);

            let header = PciHeader::new(address);
            if header.has_multiple_functions(self) {
                /*
                 * The device is multi-function. We need to check the rest.
                 */
                for function in 1..8 {
                    self.check_function(bus, device, function);
                }
            }
        }
    }

    fn check_function(&self, bus: u8, device: u8, function: u8) {
        let address = PciAddress { segment: 0, bus, device, function };
        if self.function_exists(address) {
            let header = PciHeader::new(address);
            let (vendor_id, device_id) = header.id(self);

            if vendor_id == 0xffff {
                return;
            }

            // TODO: check if the function is a PCI-to-PCI bridge, and if so, call check_bus on the secondary bus
            // number (from the bridge's config space)

            info!(
                "Found PCI device (bus={}, device={}, function={}): (vendor = {:#x}, device = {:#x})",
                bus, device, function, vendor_id, device_id
            );
        }
    }
}
