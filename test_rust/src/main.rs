#![feature(lang_items)]
#![feature(asm)]
#![feature(panic_implementation)]
#![no_std]
#![no_main]

use core::panic::PanicInfo;

// const MESSAGE : &'static str = "Hello from no-std Rust!";

#[no_mangle]
pub extern "C" fn _start() -> ! {
    unsafe {
        asm!("mov rax, 0xdeadbeef"
             :
             :
             : "rax"
             : "intel", "volatile");
        // asm!("mov rdi, 1
        //       mov rbx, 20
        //       int 0x80"
        //      :
        //      : "rax"(&MESSAGE)
        //      : "rdi", "rbx"
        //      : "intel", "volatile");
    }
    loop {}
}

#[panic_implementation]
#[no_mangle]
pub extern "C" fn rust_begin_panic(_info: &PanicInfo) -> ! {
    loop {}
}
