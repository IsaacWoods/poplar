#![feature(asm)]
#![feature(panic_implementation)]
#![no_std]
#![no_main]

extern crate libmessage;

use core::panic::PanicInfo;
use libmessage::kernel::KernelMessage;
use libmessage::buffers::SendBuffer;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let send_buffer = unsafe { SendBuffer::new() };
    let message = KernelMessage::A;
    send_buffer.send(&message).unwrap();
    unsafe { asm!("int 0x80" :::: "intel"); }

    loop {}
}

#[panic_implementation]
#[no_mangle]
pub extern "C" fn rust_begin_panic(_info: &PanicInfo) -> ! {
    loop {}
}
