#![no_std]
#![no_main]
#![feature(asm)]

use core::panic::PanicInfo;
use libpebble::syscall;

#[no_mangle]
pub extern "C" fn start() -> ! {
    syscall::yield_to_kernel();
    loop {}
}

#[panic_handler]
pub fn handle_panic(_: &PanicInfo) -> ! {
    loop {}
}
