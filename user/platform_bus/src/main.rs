#![no_std]
#![no_main]
#![feature(const_generics, alloc_error_handler)]

extern crate alloc;
extern crate rlibc;

use alloc::{collections::BTreeMap, rc::Rc, string::String, vec::Vec};
use core::{convert::TryFrom, mem, panic::PanicInfo};
use libpebble::{
    caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING, CAP_SERVICE_PROVIDER},
    channel::Channel,
    early_logger::EarlyLogger,
    syscall,
    syscall::GetMessageError,
    Handle,
};
use linked_list_allocator::LockedHeap;
use log::{info, warn};
use platform_bus::{BusDriverMessage, DeviceDriverMessage, DeviceDriverRequest, DeviceInfo, Filter, Property};

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

type BusDriverIndex = usize;
type DeviceDriverIndex = usize;

#[derive(Debug)]
enum DeviceState {
    Unclaimed(DeviceInfo),
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

#[no_mangle]
pub extern "C" fn _start() -> ! {
    syscall::early_log("Hello from platform_bus!").unwrap();
    // Initialise the heap
    const HEAP_START: usize = 0x600000000;
    const HEAP_SIZE: usize = 0x4000;
    let heap_memory_object = syscall::create_memory_object(HEAP_START, HEAP_SIZE, true, false).unwrap();
    unsafe {
        syscall::map_memory_object(&heap_memory_object, &libpebble::ZERO_HANDLE, None, 0x0 as *mut usize).unwrap();
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

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
                        BusDriverMessage::RegisterDevice(name, info) => {
                            info!("Registering device: {:?} as {}", info, name);
                            devices.insert(
                                name,
                                DeviceEntry { bus_driver: *index, state: DeviceState::Unclaimed(info) },
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
                    Some(message) => match message {
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
                    },
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
                        DeviceState::Unclaimed(ref info) => {
                            matches_so_far && filter.match_against(&info.properties)
                        }
                        _ => false,
                    },
                );

                if matches_filter {
                    info!("Found a match for device: {:?}!", name);

                    let old_state = mem::replace(&mut device.state, DeviceState::Claimed(*index));
                    match old_state {
                        DeviceState::Unclaimed(info) => {
                            device_driver
                                .channel
                                .send(&DeviceDriverRequest::HandoffDevice(name.clone(), info))
                                .unwrap();
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
    }
}

#[panic_handler]
pub fn handle_panic(info: &PanicInfo) -> ! {
    log::error!("PANIC: {}", info);
    loop {}
}

#[alloc_error_handler]
fn alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Alloc error: {:?}", layout);
}

#[used]
#[link_section = ".caps"]
pub static mut CAPS: CapabilitiesRepr<4> =
    CapabilitiesRepr::new([CAP_EARLY_LOGGING, CAP_SERVICE_PROVIDER, CAP_PADDING, CAP_PADDING]);
