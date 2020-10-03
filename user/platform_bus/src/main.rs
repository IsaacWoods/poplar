#![no_std]
#![no_main]
#![feature(const_generics, alloc_error_handler)]

extern crate alloc;
extern crate rlibc;

use alloc::{collections::BTreeMap, string::String, vec::Vec};
use core::{convert::TryFrom, panic::PanicInfo};
use libpebble::{
    caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING, CAP_SERVICE_PROVIDER},
    early_logger::EarlyLogger,
    syscall,
    syscall::GetMessageError,
    Handle,
};
use linked_list_allocator::LockedHeap;
use log::info;
use platform_bus::{BusDriverMessage, Device, Property};

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

struct DeviceEntry {
    device: Device,
    /// Handle to the channel we have to the Bus Driver
    bus_driver: Handle,
    /// If this is `None`, the device has not been claimed. If this is `Some`, the handle points to the driver that
    /// manages this device.
    device_driver: Option<Handle>,
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    syscall::early_log("Hello from platform_bus!").unwrap();
    // Initialise the heap
    const HEAP_START: usize = 0x600000000;
    const HEAP_SIZE: usize = 0x4000;
    let heap_memory_object = syscall::create_memory_object(HEAP_START, HEAP_SIZE, true, false).unwrap();
    unsafe {
        syscall::map_memory_object(heap_memory_object, libpebble::ZERO_HANDLE, 0x0 as *mut usize).unwrap();
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    log::set_logger(&EarlyLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("Platform-bus is running!");

    let bus_driver_service_channel = syscall::register_service("bus_driver").unwrap();
    let mut bus_drivers = Vec::new();
    let mut devices = BTreeMap::<String, DeviceEntry>::new();

    loop {
        syscall::yield_to_kernel();

        /*
         * Register any new bus drivers that want a channel to register devices.
         */
        {
            let mut handles = [libpebble::ZERO_HANDLE; 1];
            match syscall::get_message(bus_driver_service_channel, &mut [], &mut handles) {
                Ok((bytes, handles)) => {
                    info!("Bus driver subscribed to Platform Bus 'bus_driver' service");
                    bus_drivers.push(handles[0]);
                }
                Err(GetMessageError::NoMessage) => (),
                Err(err) => panic!("Error getting message from service subscriber: {:?}", err),
            }
        }

        /*
         * Listen to Bus Driver channels to see if any of them have sent us any messages.
         */
        for bus_driver in bus_drivers.iter() {
            loop {
                let mut bytes = [0u8; 256];
                match syscall::get_message(*bus_driver, &mut bytes, &mut []) {
                    Ok((bytes, _)) => {
                        let message = ptah::from_wire::<BusDriverMessage>(&bytes)
                            .expect("Message from bus driver is malformed");
                        match message {
                            BusDriverMessage::RegisterDevice(name, device) => {
                                info!("Registering device: {:?} as {}", device, name);
                                devices.insert(
                                    name,
                                    DeviceEntry { device, bus_driver: *bus_driver, device_driver: None },
                                );
                            }
                            BusDriverMessage::AddProperty(name, property) => todo!(),
                            BusDriverMessage::RemoveProperty(name) => todo!(),
                        }
                    }
                    Err(GetMessageError::NoMessage) => break,
                    Err(err) => panic!("Failed getting message from bus driver: {:?}", err),
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
