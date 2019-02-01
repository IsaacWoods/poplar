#![no_std]
#![no_main]
#![feature(asm)]

use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn start() -> ! {
    unsafe {
        asm!("mov rax, 0xdeadbeef" :::: "intel");
        asm!("syscall");
        asm!("mov rax, 0xcafebabe" :::: "intel");
    }
    loop {}
}

#[panic_handler]
pub fn handle_panic(_: &PanicInfo) -> ! {
    loop {}
}
