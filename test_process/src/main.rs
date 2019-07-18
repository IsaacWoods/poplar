#![no_std]
#![no_main]
#![feature(asm)]

use core::panic::PanicInfo;
use libpebble::syscall;

#[no_mangle]
pub extern "C" fn start() -> ! {
    syscall::early_log("Hello, World!").unwrap();
    syscall::yield_to_kernel();
    syscall::early_log("After yeild").unwrap();
    loop {}
}

#[panic_handler]
pub fn handle_panic(_: &PanicInfo) -> ! {
    // We ignore the result here because there's no point panicking in the panic handler
    let _ = syscall::early_log("Test process panicked!");
    loop {}
}
