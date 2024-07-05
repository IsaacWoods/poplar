mod pci;

use log::{info, warn};
use platform_bus::{BusDriverMessage, DeviceDriverMessage, DeviceDriverRequest, DeviceInfo, Filter, HandoffInfo};
use spinning_top::RwSpinlock;
use std::{
    collections::BTreeMap,
    mem,
    poplar::{
        caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING, CAP_PCI_BUS_DRIVER, CAP_SERVICE_PROVIDER},
        channel::Channel,
        early_logger::EarlyLogger,
    },
    sync::Arc,
};

type BusDriverIndex = usize;
type DeviceDriverIndex = usize;

/// Denotes that a device has been added to the Platform Bus directly from information provided to
/// us by the kernel. This is required because things like PCI devices or devices described by
/// the device tree, for example, are managed by the Platform Bus directly.
pub const KERNEL_DEVICE: BusDriverIndex = usize::MAX;

struct BusDriver {
    channel: Arc<Channel<(), BusDriverMessage>>,
}

struct DeviceDriver {
    /// If this is `None`, the driver hasn't registered its filters yet, and shouldn't be offered any devices.
    filters: Option<Vec<Filter>>,
    channel: Arc<Channel<DeviceDriverRequest, DeviceDriverMessage>>,
}

#[derive(Debug)]
pub enum Device {
    Unclaimed { bus_driver: BusDriverIndex, device_info: DeviceInfo, handoff_info: HandoffInfo },
    Claimed { bus_driver: BusDriverIndex, device_info: DeviceInfo, device_driver: DeviceDriverIndex },
}

impl Device {
    pub fn is_claimed(&self) -> bool {
        match self {
            Device::Unclaimed { .. } => false,
            Device::Claimed { .. } => true,
        }
    }
}

struct PlatformBus {
    pub bus_drivers: RwSpinlock<Vec<BusDriver>>,
    pub device_drivers: RwSpinlock<Vec<DeviceDriver>>,
    pub devices: RwSpinlock<BTreeMap<String, Device>>,
}

impl PlatformBus {
    pub fn new() -> Arc<PlatformBus> {
        Arc::new(PlatformBus {
            bus_drivers: RwSpinlock::new(Vec::new()),
            device_drivers: RwSpinlock::new(Vec::new()),
            devices: RwSpinlock::new(BTreeMap::new()),
        })
    }

    // TODO: not convinced the channels should be Arc'd
    pub fn register_bus_driver(&self, channel: Arc<Channel<(), BusDriverMessage>>) -> BusDriverIndex {
        let mut bus_drivers = self.bus_drivers.write();
        let index = bus_drivers.len();
        bus_drivers.push(BusDriver { channel });
        index
    }

    // TODO: not convinced the channels should be Arc'd
    pub fn register_device_driver(
        &self,
        channel: Arc<Channel<DeviceDriverRequest, DeviceDriverMessage>>,
    ) -> DeviceDriverIndex {
        let mut device_drivers = self.device_drivers.write();
        let index = device_drivers.len();
        device_drivers.push(DeviceDriver { filters: None, channel });
        index
    }

    pub fn register_device(&self, name: String, device: Device) {
        let mut devices = self.devices.write();
        devices.insert(name, device);
    }

    /// Check if any unclaimed devices match the filters for any device drivers, and if so query
    /// the driver for support. This should be called whenever a change is detected that could mean
    /// a device could be handed off (e.g. a new device is registered, or a device driver registers
    /// its interest).
    pub fn check_devices(&self) {
        for (name, device) in self.devices.write().iter_mut() {
            // Skip devices that have already been handed off.
            if let Device::Claimed { .. } = device {
                continue;
            }

            let device_drivers = self.device_drivers.read();
            for device_driver in device_drivers.iter().filter(|driver| driver.filters.is_some()) {
                let mut matches_filter = false;
                for filter in device_driver.filters.as_ref().unwrap() {
                    match device {
                        Device::Unclaimed { ref device_info, .. } => {
                            if filter.match_against(&device_info.0) {
                                matches_filter = true;
                                break;
                            }
                        }
                        _ => (),
                    }
                }

                if matches_filter {
                    info!("Asking device driver with matching filter if it can handle device {}", name);
                    match device {
                        Device::Unclaimed { device_info, .. } => {
                            device_driver
                                .channel
                                .send(&DeviceDriverRequest::QuerySupport(name.clone(), device_info.clone()))
                                .unwrap();
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

pub fn main() {
    log::set_logger(&EarlyLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("PlatformBus is running!");

    // TODO: this should probs be replaced with a macro similar to `tokio::main`
    std::poplar::rt::init_runtime();

    let bus_driver_service_channel = Channel::register_service("bus_driver").unwrap();
    let device_driver_service_channel = Channel::register_service("device_driver").unwrap();

    let platform_bus = PlatformBus::new();

    /*
     * Add devices from buses that the Platform Bus enumerates itself.
     */
    platform_bus.devices.write().append(&mut pci::enumerate_pci_devices());

    /*
     * Listen for new bus drivers that want a channel to register devices.
     */
    std::poplar::rt::spawn({
        let platform_bus = platform_bus.clone();
        async move {
            loop {
                let bus_driver_handle = bus_driver_service_channel.receive().await.unwrap();

                info!("Bus driver subscribed to PlatformBus!");
                let channel = Arc::new(Channel::new_from_handle(bus_driver_handle));
                let bus_driver_index = platform_bus.register_bus_driver(channel.clone());

                /*
                 * Each new bus driver gets a task to listen for newly registered devices.
                 */
                std::poplar::rt::spawn({
                    let platform_bus = platform_bus.clone();
                    async move {
                        loop {
                            match channel.receive().await.unwrap() {
                                BusDriverMessage::RegisterDevice(name, device_info, handoff_info) => {
                                    info!(
                                        "Registering device: Device: {:?}, Handoff: {:?} as {}",
                                        device_info, handoff_info, name
                                    );
                                    platform_bus.register_device(
                                        name,
                                        Device::Unclaimed {
                                            bus_driver: bus_driver_index,
                                            device_info,
                                            handoff_info,
                                        },
                                    );
                                    platform_bus.check_devices();
                                }
                            }
                        }
                    }
                });
            }
        }
    });

    /*
     * Listen for new device drivers that want a channel to claim devices on.
     */
    std::poplar::rt::spawn(async move {
        loop {
            let device_driver_handle = device_driver_service_channel.receive().await.unwrap();

            info!("Device driver subscribed to PlatformBus!");
            let channel = Arc::new(Channel::new_from_handle(device_driver_handle));
            let device_driver_index = platform_bus.register_device_driver(channel.clone());

            /*
             * Each new device driver gets a task to listen for newly registered devices.
             */
            let platform_bus = platform_bus.clone();
            std::poplar::rt::spawn(async move {
                loop {
                    match channel.receive().await.unwrap() {
                        DeviceDriverMessage::RegisterInterest(filters) => {
                            info!("Registering interest for devices with filters: {:?}", filters);
                            {
                                let mut device_drivers = platform_bus.device_drivers.write();
                                let device_driver = device_drivers.get_mut(device_driver_index).unwrap();

                                /*
                                 * We only allow device drivers to register their interests once. After that, we just
                                 * ignore them.
                                 */
                                if device_driver.filters.is_none() {
                                    device_driver.filters = Some(filters);
                                } else {
                                    warn!("Device driver tried to register interests more than one. Ignored.");
                                }
                            }
                            platform_bus.check_devices();
                        }
                        DeviceDriverMessage::CanSupport(device_name, does_support) => {
                            if does_support {
                                let mut device_drivers = platform_bus.device_drivers.write();
                                let device_driver = device_drivers.get_mut(device_driver_index).unwrap();
                                let mut devices = platform_bus.devices.write();
                                let device = devices.get_mut(&device_name).unwrap();

                                if device.is_claimed() {
                                    warn!("Device driver claimed support for '{}', but device has already been handed off! Ignoring.", device_name);
                                    continue;
                                }

                                info!("Handing off device '{}' to supporting device driver", device_name);
                                let claimed_device =
                                    if let Device::Unclaimed { bus_driver, device_info, .. } = &device {
                                        Device::Claimed {
                                            bus_driver: *bus_driver,
                                            device_info: device_info.clone(),
                                            device_driver: device_driver_index,
                                        }
                                    } else {
                                        panic!()
                                    };
                                let taken_device = mem::replace(device, claimed_device);
                                if let Device::Unclaimed { bus_driver, device_info, handoff_info } = taken_device {
                                    device_driver
                                        .channel
                                        .send(&DeviceDriverRequest::HandoffDevice(
                                            device_name,
                                            device_info.clone(),
                                            handoff_info,
                                        ))
                                        .unwrap();
                                } else {
                                    panic!();
                                }
                            }
                        }
                    }
                }
            });
        }
    });

    std::poplar::rt::enter_loop();
}

#[used]
#[link_section = ".caps"]
pub static mut CAPS: CapabilitiesRepr<4> =
    CapabilitiesRepr::new([CAP_EARLY_LOGGING, CAP_SERVICE_PROVIDER, CAP_PCI_BUS_DRIVER, CAP_PADDING]);
