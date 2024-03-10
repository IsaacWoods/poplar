use alloc::vec::Vec;
use bit_field::BitField;
use core::ptr;
use fdt::Fdt;
use pci_types::{
    device_type::DeviceType,
    Bar,
    CommandRegister,
    ConfigRegionAccess,
    EndpointHeader,
    HeaderType,
    PciAddress,
    PciHeader,
};
use tracing::info;

pub struct PciResolver {
    ecam_base: *const u8,
    ecam_size: usize,

    ranges: Vec<HostMemoryRange>,
}

impl PciResolver {
    pub fn initialize(fdt: &Fdt) {
        let mut resolver = {
            let Some(pci_node) = fdt
                .all_nodes()
                .filter(|node| {
                    node.compatible().map_or(false, |c| {
                        c.all().any(|c| ["pci-host-ecam-generic", "pci-host-cam-generic"].contains(&c))
                    })
                })
                .next()
            else {
                return;
            };
            let ecam_window = pci_node.reg().expect("PCI entry doesn't have a reg property").next().unwrap();

            let ranges = pci_node
                .ranges()
                .unwrap()
                .into_iter()
                .map(|range| {
                    /*
                     * The PCI address is encoded into the high bits of the child bus address. The
                     * encoding is explained here: https://elinux.org/Device_Tree_Usage.
                     */
                    let child_bus_address_hi = range.child_bus_address_hi;
                    let space = match child_bus_address_hi.get_bits(24..26) {
                        0b00 => AddressSpace::Config,
                        0b01 => AddressSpace::Io,
                        0b10 => AddressSpace::Memory32,
                        0b11 => AddressSpace::Memory64,
                        _ => unreachable!(),
                    };
                    let bus_address = PciAddress::new(
                        0,
                        child_bus_address_hi.get_bits(16..24) as u8,
                        child_bus_address_hi.get_bits(11..16) as u8,
                        child_bus_address_hi.get_bits(8..11) as u8,
                    );
                    let reg = child_bus_address_hi.get_bits(0..8) as u8;
                    HostMemoryRange::new(space, bus_address, reg, range.parent_bus_address, range.size)
                })
                .collect();

            Self { ecam_base: ecam_window.starting_address, ecam_size: ecam_window.size.unwrap(), ranges }
        };

        /*
         * If the device at 0:0:0:0 has multiple functions, there are multiple PCI host controllers, so we need to
         * check all the functions.
         */
        if PciHeader::new(PciAddress::new(0, 0, 0, 0)).has_multiple_functions(&resolver) {
            for bus in 0..8 {
                resolver.check_bus(bus);
            }
        } else {
            resolver.check_bus(0);
        }
    }

    fn check_bus(&mut self, bus: u8) {
        for device in 0..32 {
            self.check_device(bus, device);
        }
    }

    fn check_device(&mut self, bus: u8, device: u8) {
        let address = PciAddress::new(0, bus, device, 0);
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

    fn check_function(&mut self, bus: u8, device: u8, function: u8) {
        let address = PciAddress::new(0, bus, device, function);

        if self.function_exists(address) {
            let header = PciHeader::new(address);
            let (vendor_id, device_id) = header.id(self);
            let (_revision, class, sub_class, _interface) = header.revision_and_class(self);

            if vendor_id == 0xffff {
                return;
            }

            info!(
                "Found PCI device (bus={}, device={}, function={}): (vendor = {:#x}, device = {:#x}, class={:#x}, subclass={:#x}) -> {:?}",
                bus,
                device,
                function,
                vendor_id,
                device_id,
                class,
                sub_class,
                DeviceType::from((class, sub_class))
            );

            match header.header_type(self) {
                HeaderType::Endpoint => {
                    let mut endpoint_header = EndpointHeader::from_header(header, self).unwrap();
                    let bars = {
                        let mut bars = [None; 6];

                        let mut skip_next = false;
                        for i in 0..6 {
                            if skip_next {
                                continue;
                            }

                            let bar = endpoint_header.bar(i, self);
                            skip_next = match bar {
                                Some(Bar::Memory64 { .. }) => true,
                                _ => false,
                            };
                            bars[i as usize] = bar;
                        }

                        bars
                    };

                    /*
                     * It's our responsibility to allocate memory for the BARs of PCI devices on
                     * RISC-V. This memory needs to be addressable by both the CPU and the PCI host
                     * bridge, and conform to the device's requirements, so we have to choose a
                     * suitable region from the reported ranges.
                     */
                    let mut needs_memory_access = false;
                    for (i, bar) in bars.iter().enumerate() {
                        if let Some(bar) = *bar {
                            let address = self
                                .ranges
                                .iter_mut()
                                .find_map(|range| match bar {
                                    Bar::Memory32 { size, .. } => {
                                        if range.space != AddressSpace::Memory32 {
                                            return None;
                                        }
                                        needs_memory_access = true;
                                        range.allocate(size as usize, size as usize)
                                    }
                                    Bar::Memory64 { size, .. } => {
                                        if range.space != AddressSpace::Memory64 {
                                            return None;
                                        }
                                        needs_memory_access = true;
                                        range.allocate(size as usize, size as usize)
                                    }
                                    Bar::Io { .. } => unimplemented!(),
                                })
                                .expect("Failed to allocate memory for BAR");
                            unsafe {
                                endpoint_header.write_bar(i as u8, self, address).unwrap();
                            }
                        }
                    }

                    endpoint_header.update_command(self, |mut command| {
                        command |= CommandRegister::BUS_MASTER_ENABLE;

                        if needs_memory_access {
                            command |= CommandRegister::MEMORY_ENABLE;
                        }
                        command
                    });
                }

                HeaderType::PciPciBridge => {
                    // TODO: call check_bus on the bridge's secondary bus number
                    todo!()
                }

                HeaderType::CardBusBridge => {
                    // TODO: what do we even do with these?
                    todo!()
                }

                reserved => panic!("PCI function has reserved header type: {:?}", reserved),
            }
        }
    }

    fn address_for(&self, pci_address: PciAddress) -> *const u8 {
        unsafe {
            self.ecam_base.add(
                usize::from(pci_address.bus()) << 20
                    | usize::from(pci_address.device()) << 15
                    | usize::from(pci_address.function()) << 12,
            )
        }
    }
}

impl ConfigRegionAccess for PciResolver {
    fn function_exists(&self, _address: PciAddress) -> bool {
        // TODO
        true
    }

    unsafe fn read(&self, address: PciAddress, offset: u16) -> u32 {
        ptr::read_volatile(self.address_for(address).add(offset as usize) as *const u32)
    }

    unsafe fn write(&self, address: PciAddress, offset: u16, value: u32) {
        ptr::write_volatile(self.address_for(address).add(offset as usize) as *mut u32, value);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
enum AddressSpace {
    Config = 0,
    Io = 1,
    Memory32 = 2,
    Memory64 = 3,
}

#[derive(Clone, Debug)]
struct HostMemoryRange {
    space: AddressSpace,
    address: PciAddress,
    reg: u8,
    cpu_base: usize,
    cpu_size: usize,
    cursor: usize,
}

impl HostMemoryRange {
    pub fn new(
        space: AddressSpace,
        address: PciAddress,
        reg: u8,
        cpu_base: usize,
        cpu_size: usize,
    ) -> HostMemoryRange {
        HostMemoryRange { space, address, reg, cpu_base, cpu_size, cursor: 0 }
    }

    pub fn allocate(&mut self, size: usize, alignment: usize) -> Option<usize> {
        let padding = alignment.wrapping_sub(self.cursor) & (alignment - 1);
        if (size + padding) > (self.cpu_size - self.cursor) {
            return None;
        }

        let base = self.cpu_base + self.cursor + padding;
        self.cursor = base + size;

        Some(base)
    }
}
