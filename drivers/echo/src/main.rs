#![no_std]
#![no_main]
#![feature(const_generics, alloc_error_handler)]

extern crate rlibc;

use core::panic::PanicInfo;
use libpebble::{
    caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING},
    early_logger::EarlyLogger,
    syscall,
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

    loop {
        info!("Echo loop");
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
    CapabilitiesRepr::new([CAP_EARLY_LOGGING, CAP_PADDING, CAP_PADDING, CAP_PADDING]);
