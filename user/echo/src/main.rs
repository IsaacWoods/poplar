#![no_std]
#![no_main]
#![feature(const_generics, alloc_error_handler, never_type)]

extern crate alloc;
extern crate rlibc;

use alloc::vec::Vec;
use core::panic::PanicInfo;
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

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[no_mangle]
pub extern "C" fn _start() -> ! {
    syscall::early_log("Hello, World!").unwrap();
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
    info!("Echo running!");

    let echo_service_channel = Channel::register_service("echo").unwrap();
    let mut subscribers = Vec::new();

    loop {
        syscall::yield_to_kernel();

        /*
         * Check if any of our subscribers have sent us any messages, and if they have, echo them back.
         * NOTE: we don't support handles.
         */
        for subscriber in subscribers.iter() {
            let mut bytes = [0u8; 256];
            loop {
                match syscall::get_message(subscriber, &mut bytes, &mut []) {
                    Ok((bytes, _handles)) => {
                        info!("Echoing message: {:x?}", bytes);
                        syscall::send_message(subscriber, bytes, &[]).unwrap();
                    }
                    Err(GetMessageError::NoMessage) => break,
                    Err(err) => panic!("Error while echoing message: {:?}", err),
                }
            }
        }

        if let Some(subscriber_handle) = echo_service_channel.try_receive().unwrap() {
            info!("Task subscribed to our service!");
            subscribers.push(subscriber_handle);
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
