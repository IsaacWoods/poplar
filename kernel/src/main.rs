#![no_std]
#![cfg_attr(not(test), no_main)]

// extern crate alloc;
// #[cfg(test)]
// extern crate std;

use hal::io::IoPort;

#[unsafe(no_mangle)]
pub fn kentry() -> ! {
    unsafe {
        let debug_port = IoPort::new(0xe9);
        for c in "Hello from the kernel!".bytes() {
            debug_port.write(c);
        }
    }

    loop {}
}

#[cfg(not(test))]
#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
    // TODO
    loop {}
}
