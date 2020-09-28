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
pub extern "C" fn start() -> ! {
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

    loop {
        syscall::yield_to_kernel();
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
