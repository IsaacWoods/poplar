#![no_std]
#![no_main]
#![feature(asm, const_generics)]

use core::panic::PanicInfo;
use libpebble::{
    caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING},
    syscall,
};

#[no_mangle]
pub extern "C" fn start() -> ! {
    syscall::early_log("Hello, World! From test process").unwrap();
    loop {
        syscall::early_log("Yielding from test").unwrap();
        syscall::yield_to_kernel();
    }
}

#[panic_handler]
pub fn handle_panic(_: &PanicInfo) -> ! {
    // We ignore the result here because there's no point panicking in the panic handler
    let _ = syscall::early_log("Test process panicked!");
    loop {}
}

#[used]
#[link_section = ".caps"]
pub static mut CAPS: CapabilitiesRepr<4> =
    CapabilitiesRepr::new([CAP_EARLY_LOGGING, CAP_PADDING, CAP_PADDING, CAP_PADDING]);
