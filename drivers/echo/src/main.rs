#![no_std]
#![no_main]
#![feature(const_generics, alloc_error_handler)]

extern crate rlibc;

use core::panic::PanicInfo;
use libpebble::{
    caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING, CAP_SERVICE_PROVIDER},
    early_logger::EarlyLogger,
    syscall,
    syscall::GetMessageError,
};
use linked_list_allocator::LockedHeap;
use log::info;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[no_mangle]
pub extern "C" fn start() -> ! {
    syscall::early_log("Hello, World!").unwrap();
    // Initialise the heap
    const HEAP_START: usize = 0x600000000;
    const HEAP_SIZE: usize = 0x4000;
    let heap_memory_object = syscall::create_memory_object(HEAP_START, HEAP_SIZE, true, false).unwrap();
    syscall::map_memory_object(heap_memory_object, libpebble::ZERO_HANDLE, 0x0 as *mut usize).unwrap();
    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    log::set_logger(&EarlyLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("Echo running!");

    let echo_service_channel = syscall::register_service("echo").unwrap();
    loop {
        syscall::yield_to_kernel();

        let mut bytes = [0u8; 256];
        let mut handles = [libpebble::ZERO_HANDLE; 4];
        match syscall::get_message(echo_service_channel, &mut bytes, &mut handles) {
            Ok((bytes, handles)) => {
                info!("Got message: {:#x?} (with {} handles)!", bytes, handles.len());
            }
            Err(GetMessageError::NoMessage) => info!("No messages yet :("),
            Err(err) => panic!("Error getting message: {:?}", err),
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
