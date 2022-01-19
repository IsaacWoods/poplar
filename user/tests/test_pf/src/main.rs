#![no_std]
#![no_main]
#![feature(alloc_error_handler, thread_local)]

extern crate alloc;

use core::panic::PanicInfo;
use linked_list_allocator::LockedHeap;
use log::info;
use poplar::{
    caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING},
    early_logger::EarlyLogger,
    syscall,
};

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[no_mangle]
pub extern "C" fn _start() -> ! {
    syscall::early_log("Hello from test_pf").unwrap();
    // Initialise the heap
    const HEAP_START: usize = 0x6_0000_0000;
    const HEAP_SIZE: usize = 0x4000;
    let heap_memory_object =
        syscall::create_memory_object(HEAP_START, HEAP_SIZE, true, false, 0x0 as *mut usize).unwrap();
    unsafe {
        syscall::map_memory_object(&heap_memory_object, &poplar::ZERO_HANDLE, None, 0x0 as *mut usize).unwrap();
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    log::set_logger(&EarlyLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("test_pf is running");

    // XXX: this should cause a page fault
    // unsafe {
    //     core::ptr::read_volatile(0x8_0000_0000 as *const u8);
    // }

    for i in 0..1000 {
        info!("Loop: {}", i);
        syscall::yield_to_kernel();
    }
    loop {
        info!("Yielding from test_pf");
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
