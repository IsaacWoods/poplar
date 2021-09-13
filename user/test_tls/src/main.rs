#![no_std]
#![no_main]
#![feature(asm, alloc_error_handler, thread_local)]

extern crate alloc;

use core::{
    cell::RefCell,
    panic::PanicInfo,
    sync::atomic::{AtomicUsize, Ordering},
};
use libpebble::{
    caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_GET_FRAMEBUFFER, CAP_PADDING, CAP_SERVICE_USER},
    early_logger::EarlyLogger,
    syscall,
};
use linked_list_allocator::LockedHeap;
use log::info;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[thread_local]
static FOO: AtomicUsize = AtomicUsize::new(0);
#[thread_local]
static BAR: RefCell<u32> = RefCell::new(0);

#[no_mangle]
pub extern "C" fn _start() -> ! {
    syscall::early_log("Hello from test_tls").unwrap();
    // Initialise the heap
    const HEAP_START: usize = 0x600000000;
    const HEAP_SIZE: usize = 0x4000;
    let heap_memory_object =
        syscall::create_memory_object(HEAP_START, HEAP_SIZE, true, false, 0x0 as *mut usize).unwrap();
    unsafe {
        syscall::map_memory_object(&heap_memory_object, &libpebble::ZERO_HANDLE, None, 0x0 as *mut usize).unwrap();
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    log::set_logger(&EarlyLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("test_tls is running");

    let fs_ptr = unsafe {
        let fs_ptr: usize;
        asm!("mov rax, fs:[0x0]", out("rax") fs_ptr);
        fs_ptr
    };
    info!("FS ptr is {:#x}", fs_ptr);

    assert_eq!(FOO.load(Ordering::SeqCst), 0);
    assert_eq!(*BAR.borrow(), 0);
    *BAR.borrow_mut() = 0xff43_67de;
    FOO.store(11, Ordering::SeqCst);
    assert_eq!(FOO.load(Ordering::SeqCst), 11);
    assert_eq!(*BAR.borrow(), 0xff43_67de);

    // TODO: we don't need to carry on running. This should call an `exit` syscall or whatever.
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
    CapabilitiesRepr::new([CAP_EARLY_LOGGING, CAP_GET_FRAMEBUFFER, CAP_SERVICE_USER, CAP_PADDING]);
