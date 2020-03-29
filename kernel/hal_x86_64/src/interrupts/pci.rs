use crate::kernel_map;
use acpi::PciConfigRegions;
use aml::{pci_routing::PciRoutingTable, AmlContext, AmlName};
use bit_field::BitField;
use hal::memory::PhysicalAddress;
use log::info;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct PciConfigHeader {
    vendor_id: u16,
    device_id: u16,
    command: u16,
    status: u16,
    revision_id: u8,
    prog_if: u8,
    subclass: u8,
    class: u8,
    cache_line_size: u8,
    latency_timer: u8,
    header_type: u8,
    bist: u8,
    // ...
}

impl PciConfigHeader {
    pub fn has_multiple_functions(&self) -> bool {
        self.header_type.get_bit(7)
    }
}

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

impl<'a> PciResolver<'a> {
    pub fn resolve(config_regions: &'a PciConfigRegions, aml_context: &mut AmlContext) -> PciInfo {
        let routing_table =
            PciRoutingTable::from_prt_path(&AmlName::from_str("\\_SB.PCI0._PRT").unwrap(), aml_context)
                .expect("Failed to parse _PRT");
        let resolver = Self { pci_info: PciInfo {}, config_regions, routing_table };

        // TODO: if the device at bus=0,device=0,function=0 has multiple functions, there are multiple PCI host
        // controllers. We need to check the bus corresponding to each function (function 0 = bus 0, function 1 =
        // bus 1 etc.)
        resolver.check_bus(0);
        resolver.pci_info
    }

    fn check_bus(&self, bus: u8) {
        for device in 0..32 {
            self.check_device(bus, device);
        }
    }

    fn check_device(&self, bus: u8, device: u8) {
        if let Some(function_0_physical_address) = self.config_regions.physical_address(0, bus, device, 0) {
            self.check_function(bus, device, 0);

            let function_0_config_space = unsafe {
                *(kernel_map::physical_to_virtual(
                    PhysicalAddress::new(function_0_physical_address as usize).unwrap(),
                )
                .ptr() as *const PciConfigHeader)
            };

            if function_0_config_space.has_multiple_functions() {
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
        if let Some(config_region_address) = self.config_regions.physical_address(0, bus, device, function) {
            let config_space = unsafe {
                *(kernel_map::physical_to_virtual(PhysicalAddress::new(config_region_address as usize).unwrap())
                    .ptr() as *const PciConfigHeader)
            };

            if config_space.vendor_id == 0xffff {
                return;
            }

            // TODO: check if the function is a PCI-to-PCI bridge, and if so, call check_bus on the secondary bus
            // number (from the bridge's config space)

            info!(
                "Found PCI device (bus={}, device={}, function={}): (vendor = {:#x}, device = {:#x})",
                bus, device, function, config_space.vendor_id, config_space.device_id
            );
        }
    }
}
