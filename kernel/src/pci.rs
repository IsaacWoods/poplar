use crate::object::interrupt::Interrupt;
use alloc::{collections::BTreeMap, sync::Arc};
use pci_types::{
    capability::{MsiCapability, MsixCapability, PciCapability},
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
    pub interrupt: Option<Arc<Interrupt>>,
}

#[derive(Clone, Debug)]
pub struct PciInfo {
    pub devices: BTreeMap<PciAddress, PciDevice>,
}

pub trait PciInterruptConfigurator {
    /// Create an `Interrupt` that is signalled when an interrupt arrives from the specified PCI
    /// device. This is used when the device does not support MSI or MSI-X interrupts. The event
    /// may be triggered when the device has not actually received an interrupt, due to interrupt
    /// pin sharing in the legacy system, and so receivers must be resilient to spurious events.
    fn configure_legacy(&self, function: PciAddress, pin: u8) -> Arc<Interrupt>;

    /// Create an `Interrupt` that is signalled when an interrupt arrives from the specified PCI
    /// device. The device must support configuration of its interrupts via the passed MSI
    /// capability.
    fn configure_msi(&self, function: PciAddress, msi: &mut MsiCapability) -> Arc<Interrupt>;

    /// Create an `Interrupt` that is signalled when an interrupt arrives from the specified PCI
    /// device. The device must support configuration of its interrupts via the passed MSI-X
    /// capability.
    fn configure_msix(&self, function: PciAddress, table_bar: Bar, msix: &mut MsixCapability) -> Arc<Interrupt>;
}

pub struct PciResolver<A>
where
    A: ConfigRegionAccess + PciInterruptConfigurator,
{
    access: A,
    info: PciInfo,
}

impl<A> PciResolver<A>
where
    A: ConfigRegionAccess + PciInterruptConfigurator,
{
    pub fn resolve(access: A) -> (A, PciInfo) {
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

        (resolver.access, resolver.info)
    }

    fn check_bus(&mut self, bus: u8) {
        for device in 0..32 {
            self.check_device(bus, device);
        }
    }

    fn check_device(&mut self, bus: u8, device: u8) {
        let address = PciAddress::new(0, bus, device, 0);
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

    fn check_function(&mut self, bus: u8, device: u8, function: u8) {
        let address = PciAddress::new(0, bus, device, function);
        let header = PciHeader::new(address);
        let (vendor_id, device_id) = header.id(&self.access);
        let (revision, class, sub_class, interface) = header.revision_and_class(&self.access);

        if vendor_id == 0xffff {
            return;
        }

        info!(
            "Found PCI device {:?} ({:x}:{:x}) -> {:?}",
            address,
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

                        if let Some(bar) = &bar {
                            match bar {
                                Bar::Memory32 { address, size, .. } => {
                                    info!("BAR{}: 32-bit memory @ {:#x}..{:#x}", i, address, address + size);
                                }
                                Bar::Memory64 { address, size, .. } => {
                                    info!("BAR{}: 64-bit memory @ {:#x}..{:#x}", i, address, address + size);
                                }
                                Bar::Io { port } => info!("BAR{}: IO @ {:#x}", i, port),
                            }
                        }

                        skip_next = match bar {
                            Some(Bar::Memory64 { .. }) => true,
                            _ => false,
                        };
                        bars[i as usize] = bar;
                    }

                    bars
                };

                /*
                 * Create an object that is triggered when an interrupt arrives for the PCI device.
                 * We try to use MSI or MSI-X if the device supports it, otherwise we have to use
                 * the shared interrupt pins.
                 */
                let interrupt = endpoint_header
                    .capabilities(&self.access)
                    .find_map(|capability| match capability {
                        PciCapability::Msi(mut msi) => Some(self.access.configure_msi(address, &mut msi)),
                        PciCapability::MsiX(mut msix) => {
                            let table_bar = bars[msix.table_bar() as usize].unwrap();
                            Some(self.access.configure_msix(address, table_bar, &mut msix))
                        }
                        _ => None,
                    })
                    .or_else(|| {
                        /*
                         * If the device does not support MSI or MSI-X, we're forced to use the
                         * legacy interrupt pins.
                         */
                        let (pin, _) = endpoint_header.interrupt(&self.access);
                        match pin {
                            0x00 => {
                                // Device does not support interrupts of any kind
                                None
                            }
                            0x01..0x05 => Some(self.access.configure_legacy(address, pin)),
                            _ => panic!("Invalid legacy interrupt pin!"),
                        }
                    });

                self.info.devices.insert(
                    address,
                    PciDevice { vendor_id, device_id, revision, class, sub_class, interface, bars, interrupt },
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
