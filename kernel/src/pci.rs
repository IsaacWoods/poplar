use alloc::collections::BTreeMap;
use pci_types::{
    device_type::DeviceType,
    Bar,
    BaseClass,
    ConfigRegionAccess,
    DeviceId,
    DeviceRevision,
    EndpointHeader,
    HeaderType,
    Interface,
    PciAddress,
    PciHeader,
    SubClass,
    VendorId,
    MAX_BARS,
};
use tracing::info;

#[derive(Clone, Debug)]
pub struct PciDevice {
    pub vendor_id: VendorId,
    pub device_id: DeviceId,
    pub revision: DeviceRevision,
    pub class: BaseClass,
    pub sub_class: SubClass,
    pub interface: Interface,
    pub bars: [Option<Bar>; MAX_BARS],
}

#[derive(Clone, Debug)]
pub struct PciInfo {
    pub devices: BTreeMap<PciAddress, PciDevice>,
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
        if PciHeader::new(PciAddress::new(0, 0, 0, 0)).has_multiple_functions(&resolver.access) {
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
            let (revision, class, sub_class, interface) = header.revision_and_class(&self.access);

            if vendor_id == 0xffff {
                return;
            }

            info!(
                "Found PCI device (bus={}, device={}, function={}): (vendor = {:#x}, device = {:#x}) -> {:?}",
                bus,
                device,
                function,
                vendor_id,
                device_id,
                DeviceType::from((class, sub_class))
            );

            match header.header_type(&self.access) {
                HeaderType::Endpoint => {
                    let endpoint_header = EndpointHeader::from_header(header, &self.access).unwrap();
                    let bars = {
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

                    self.info.devices.insert(
                        address,
                        PciDevice { vendor_id, device_id, revision, class, sub_class, interface, bars },
                    );
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
