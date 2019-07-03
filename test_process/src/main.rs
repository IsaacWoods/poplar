#![no_std]
#![no_main]
#![feature(asm)]

use core::panic::PanicInfo;
use libpebble::syscall;

// XXX: this doesn't do anything, just forces the image to generate a segment to put .data in
static mut FOO: u8 = 7;

#[no_mangle]
pub extern "C" fn start() -> ! {
    unsafe {
        FOO = 11;
    }
    syscall::yield_to_kernel();
    unsafe {
        FOO = 46;
    }
    loop {}
}

#[panic_handler]
pub fn handle_panic(_: &PanicInfo) -> ! {
    loop {}
}
