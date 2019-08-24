#![no_std]
#![no_main]
#![feature(const_generics)]

use core::panic::PanicInfo;
use libpebble::syscall;

#[no_mangle]
pub extern "C" fn start() -> ! {
    let mut a = 43;
    syscall::early_log("Hello, World!").unwrap();
    a += 3;
    syscall::early_log("This is from the simple_fb driver").unwrap();
    a += 2;

    if a == 48 {
        syscall::early_log("Woooo it worked!!").unwrap();
    } else {
        syscall::early_log("Something went wrong :(").unwrap();
    }

    syscall::yield_to_kernel();
    syscall::early_log("After yield!").unwrap();
    syscall::yield_to_kernel();
    syscall::early_log("Back again!").unwrap();
    loop {}
}

#[panic_handler]
pub fn handle_panic(_: &PanicInfo) -> ! {
    // We ignore the result here because there's no point panicking in the panic handler
    let _ = syscall::early_log("Test process panicked!");
    loop {}
}

/// `N` must be a multiple of 4, and padded with zeros, so the whole descriptor is aligned to a
/// 4-byte boundary.
#[repr(C)]
pub struct Capabilities<const N: usize> {
    name_size: u32,
    desc_size: u32,
    entry_type: u32,
    name: [u8; 8],
    desc: [u8; N],
}

#[used]
#[link_section = ".caps"]
pub static mut CAPS: Capabilities<4> = Capabilities {
    name_size: 6,
    desc_size: 2,
    entry_type: 0,
    name: [b'P', b'E', b'B', b'B', b'L', b'E', b'\0', 0x00],
    desc: [0x30, 0x31, 0x00, 0x00],
};
