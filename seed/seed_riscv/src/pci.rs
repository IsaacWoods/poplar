use core::ptr;
use fdt::Fdt;
use pci_types::{
    device_type::DeviceType,
    Bar,
    ConfigRegionAccess,
    EndpointHeader,
    HeaderType,
    PciAddress,
    PciHeader,
};
use tracing::info;

pub struct PciAccess {
    start: *const u8,
    size: usize,
}

impl PciAccess {
    pub fn new(fdt: &Fdt) -> Option<PciAccess> {
        let pci_node = fdt
            .all_nodes()
            .filter(|node| {
                node.compatible().map_or(false, |c| {
                    c.all().any(|c| ["pci-host-ecam-generic", "pci-host-cam-generic"].contains(&c))
                })
            })
            .next()?;
        let ecam_window = pci_node.reg().expect("PCI entry doesn't have a reg property").next().unwrap();

        // TODO: parse `ranges` node

        Some(PciAccess { start: ecam_window.starting_address, size: ecam_window.size.unwrap() })
    }

    fn address_for(&self, pci_address: PciAddress) -> *const u8 {
        unsafe {
            self.start.add(
                usize::from(pci_address.bus()) << 20
                    | usize::from(pci_address.device()) << 15
                    | usize::from(pci_address.function()) << 12,
            )
        }
    }
}

unsafe impl Send for PciAccess {}

impl ConfigRegionAccess for PciAccess {
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

pub struct PciResolver<A>
where
    A: ConfigRegionAccess,
{
    access: A,
}

impl<A> PciResolver<A>
where
    A: ConfigRegionAccess,
{
    pub fn resolve(access: A) {
        let mut resolver = Self { access };

        /*
         * If the device at 0:0:0:0 has multiple functions, there are multiple PCI host controllers, so we need to
         * check all the functions.
         */
        if PciHeader::new(PciAddress::new(0, 0, 0, 0)).has_multiple_functions(&resolver.access) {
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
        let address = PciAddress::new(0, bus, device, function);

        if self.access.function_exists(address) {
            let header = PciHeader::new(address);
            let (vendor_id, device_id) = header.id(&self.access);
            let (_revision, class, sub_class, _interface) = header.revision_and_class(&self.access);

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

            match header.header_type(&self.access) {
                HeaderType::Endpoint => {
                    let endpoint_header = EndpointHeader::from_header(header, &self.access).unwrap();
                    let _bars = {
                        let mut bars = [None; 6];

                        let mut skip_next = false;
                        for i in 0..6 {
                            if skip_next {
                                continue;
                            }

                            let bar = endpoint_header.bar(i, &self.access);
                            skip_next = match bar {
                                Some(Bar::Memory64 { .. }) => true,
                                _ => false,
                            };
                            bars[i as usize] = bar;
                        }

                        bars
                    };
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
}
