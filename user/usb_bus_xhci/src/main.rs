#![no_std]
#![no_main]
#![feature(const_generics, alloc_error_handler, never_type)]

extern crate alloc;
extern crate rlibc;

use alloc::{collections::BTreeMap, rc::Rc, string::String, vec, vec::Vec};
use core::{convert::TryFrom, panic::PanicInfo};
use libpebble::{
    caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING, CAP_SERVICE_USER},
    channel::Channel,
    early_logger::EarlyLogger,
    syscall,
    syscall::GetMessageError,
    Handle,
};
use linked_list_allocator::LockedHeap;
use log::info;
use platform_bus::{BusDriverMessage, DeviceDriverMessage, DeviceDriverRequest, Filter, Property};

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[no_mangle]
pub extern "C" fn _start() -> ! {
    syscall::early_log("Hello from usb_bus_xhci!").unwrap();
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
    info!("XHCI USB Bus Driver is running!");
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
    CapabilitiesRepr::new([CAP_EARLY_LOGGING, CAP_SERVICE_USER, CAP_PADDING, CAP_PADDING]);
