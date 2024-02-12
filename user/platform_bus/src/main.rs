use log::{info, warn};
use platform_bus::{
    BusDriverMessage,
    DeviceDriverMessage,
    DeviceDriverRequest,
    DeviceInfo,
    Filter,
    HandoffInfo,
    Property,
};
use std::{
    collections::BTreeMap,
    mem,
    poplar::{
        caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING, CAP_SERVICE_PROVIDER},
        channel::Channel,
        early_logger::EarlyLogger,
        syscall,
        syscall::GetMessageError,
        Handle,
    },
    rc::Rc,
};

type BusDriverIndex = usize;
type DeviceDriverIndex = usize;

#[derive(Debug)]
enum DeviceState {
    // TODO: this is used to transistion between states, but probably shouldn't exist.
    Thinking,
    Unclaimed { device_info: DeviceInfo, handoff_info: HandoffInfo },
    Querying { driver: BusDriverIndex, device_info: DeviceInfo, handoff_info: HandoffInfo },
    Claimed(BusDriverIndex),
}

struct DeviceEntry {
    bus_driver: BusDriverIndex,
    state: DeviceState,
}

struct BusDriver {
    channel: Channel<(), BusDriverMessage>,
}

struct DeviceDriver {
    /// If this is `None`, the driver hasn't registered its filters yet, and shouldn't be offered any devices.
    filters: Option<Vec<Filter>>,
    channel: Channel<DeviceDriverRequest, DeviceDriverMessage>,
}

pub fn main() {
    log::set_logger(&EarlyLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("PlatformBus is running!");

    let bus_driver_service_channel = Channel::register_service("bus_driver").unwrap();
    let device_driver_service_channel = Channel::register_service("device_driver").unwrap();

    let mut current_bus_driver_index = 0;
    let mut current_device_driver_index = 0;

    let mut bus_drivers: Vec<(BusDriverIndex, BusDriver)> = Vec::new();
    let mut device_drivers: Vec<(DeviceDriverIndex, DeviceDriver)> = Vec::new();
    let mut devices = BTreeMap::<String, DeviceEntry>::new();

    loop {
        syscall::yield_to_kernel();

        /*
         * Register any new bus drivers that want a channel to register devices.
         */
        loop {
            if let Some(bus_driver_handle) = bus_driver_service_channel.try_receive().unwrap() {
                info!("Bus driver subscribed to PlatformBus!");
                bus_drivers.push((
                    current_bus_driver_index,
                    BusDriver { channel: Channel::from_handle(bus_driver_handle) },
                ));
                current_bus_driver_index += 1;
            } else {
                break;
            }
        }

        /*
         * Register any new device drivers.
         */
        loop {
            if let Some(device_driver_handle) = device_driver_service_channel.try_receive().unwrap() {
                info!("Device driver subscribed to PlatformBus!");
                device_drivers.push((
                    current_device_driver_index,
                    DeviceDriver { channel: Channel::from_handle(device_driver_handle), filters: None },
                ));
                current_device_driver_index += 1;
            } else {
                break;
            }
        }

        /*
         * Listen to Bus Driver channels to see if any of them have sent us any messages.
         */
        for (index, bus_driver) in bus_drivers.iter() {
            loop {
                match bus_driver.channel.try_receive().unwrap() {
                    Some(message) => match message {
                        BusDriverMessage::RegisterDevice(name, device_info, handoff_info) => {
                            info!(
                                "Registering device: Device: {:?}, Handoff: {:?} as {}",
                                device_info, handoff_info, name
                            );
                            devices.insert(
                                name,
                                DeviceEntry {
                                    bus_driver: *index,
                                    state: DeviceState::Unclaimed { device_info, handoff_info },
                                },
                            );
                        }
                    },
                    None => break,
                }
            }
        }

        /*
         * Listen to Device Driver channels to see if any of them have sent us any messages.
         */
        for (index, device_driver) in device_drivers.iter_mut() {
            loop {
                match device_driver.channel.try_receive().unwrap() {
                    Some(message) => {
                        match message {
                            DeviceDriverMessage::RegisterInterest(filters) => {
                                info!("Registering interest for devices with filters: {:?}", filters);

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
                            DeviceDriverMessage::CanSupport(device_name, does_support) => {
                                if does_support {
                                    info!("Handing off device '{}' to supporting device driver", device_name);
                                    let device = devices.get_mut(&device_name).unwrap();
                                    let old_state = mem::replace(&mut device.state, DeviceState::Claimed(*index));
                                    match old_state {
                                        DeviceState::Querying { device_info, handoff_info, .. } => {
                                            device_driver
                                                .channel
                                                .send(&DeviceDriverRequest::HandoffDevice(
                                                    device_name,
                                                    device_info,
                                                    handoff_info,
                                                ))
                                                .unwrap();
                                        }
                                        _ => panic!(),
                                    }
                                } else {
                                    // TODO: this is not handled as of yet but see note below about
                                    // async runtime-related plans
                                    warn!("Device driver says it doesn't support device '{}'. This is broken as of now!", device_name);
                                }
                            }
                        }
                    }
                    None => break,
                }
            }
        }

        /*
         * Now we've handled any new messages, check to see if we have any unclaimed devices. If we do, check to
         * see if we have a device driver to offer them to.
         */
        for (name, device) in devices.iter_mut() {
            // Skip devices that have already been handed off to a driver
            if let DeviceState::Claimed(_) = device.state {
                continue;
            }

            for (index, device_driver) in device_drivers.iter().filter(|(_, driver)| driver.filters.is_some()) {
                let matches_filter = device_driver.filters.as_ref().unwrap().iter().fold(
                    true,
                    |matches_so_far, filter| match device.state {
                        DeviceState::Unclaimed { ref device_info, .. } => {
                            matches_so_far && filter.match_against(&device_info.0)
                        }
                        _ => false,
                    },
                );

                if matches_filter {
                    info!("Asking device driver with matching filter if it can handle device {}", name);
                    let state = mem::replace(&mut device.state, DeviceState::Thinking);
                    let (device_info, handoff_info) = match state {
                        DeviceState::Unclaimed { device_info, handoff_info } => {
                            device_driver
                                .channel
                                .send(&DeviceDriverRequest::QuerySupport(name.clone(), device_info.clone()))
                                .unwrap();
                            (device_info, handoff_info)
                        }
                        _ => panic!("Uh oh more than one driver has a matching filter (this is bad for now but shouldn't be)!"),
                    };
                    device.state = DeviceState::Querying { driver: *index, device_info, handoff_info };
                    // TODO: this actually depends on the first driver saying yes lmao. Doing this
                    // properly will require more thought but will all be replaced (probably rather
                    // soonish now) by a userspace async solution I think because this is rather
                    // untenable... (with async, we'd just wait for the reply / ask each driver and
                    // track the response)
                }
            }
        }
    }
}

#[used]
#[link_section = ".caps"]
pub static mut CAPS: CapabilitiesRepr<4> =
    CapabilitiesRepr::new([CAP_EARLY_LOGGING, CAP_SERVICE_PROVIDER, CAP_PADDING, CAP_PADDING]);
