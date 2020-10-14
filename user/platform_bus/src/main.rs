#![no_std]
#![no_main]
#![feature(const_generics, alloc_error_handler)]

extern crate alloc;
extern crate rlibc;

use alloc::{collections::BTreeMap, rc::Rc, string::String, vec::Vec};
use core::{convert::TryFrom, panic::PanicInfo};
use libpebble::{
    caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING, CAP_SERVICE_PROVIDER},
    channel::Channel,
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
    bus_driver: Rc<Channel<(), BusDriverMessage>>,
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
    info!("PlatformBus is running!");

    let bus_driver_service_channel = Channel::register_service("bus_driver").unwrap();
    let mut bus_drivers: Vec<Rc<Channel<(), BusDriverMessage>>> = Vec::new();
    let mut devices = BTreeMap::<String, DeviceEntry>::new();

    loop {
        syscall::yield_to_kernel();

        /*
         * Register any new bus drivers that want a channel to register devices.
         */
        if let Some(bus_driver_handle) = bus_driver_service_channel.try_receive().unwrap() {
            info!("Bus driver subscribed to PlatformBus!");
            bus_drivers.push(Rc::new(Channel::from_handle(bus_driver_handle)));
        }

        /*
         * Listen to Bus Driver channels to see if any of them have sent us any messages.
         */
        for bus_driver in bus_drivers.iter() {
            loop {
                match bus_driver.try_receive().unwrap() {
                    Some(message) => match message {
                        BusDriverMessage::RegisterDevice(name, device) => {
                            info!("Registering device: {:?} as {}", device, name);
                            devices.insert(
                                name,
                                DeviceEntry { device, bus_driver: bus_driver.clone(), device_driver: None },
                            );
                        }
                        BusDriverMessage::AddProperty(name, property) => todo!(),
                        BusDriverMessage::RemoveProperty(name) => todo!(),
                    },
                    None => break,
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
