#![feature(asm)]
#![feature(panic_implementation)]
#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    unsafe {
        asm!("mov rax, 0xdeadbeef"
             :
             :
             : "rax"
             : "intel", "volatile");
    }

    loop {}
}

#[panic_implementation]
#[no_mangle]
pub extern "C" fn rust_begin_panic(_info: &PanicInfo) -> ! {
    loop {}
}
