#![no_std]
#![no_main]
#![feature(asm)]

use core::panic::PanicInfo;
use libpebble::{
    caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING},
    syscall,
};

#[no_mangle]
pub extern "C" fn _start() -> ! {
    syscall::early_log("Hello from test_syscalls").unwrap();

    // TODO: make some test syscalls in the kernel (maybe test for magic values in each value)
    // TODO: validate in kernel that everything is in the right place
    // TODO: validate in userspace that register values we expect to be preserved are
    // TODO: explore userspace stack canaries to see if it's faffing with that

    loop {
        syscall::early_log("Yielding").unwrap();
        syscall::yield_to_kernel();
    }
}

#[panic_handler]
pub fn handle_panic(_: &PanicInfo) -> ! {
    let _ = syscall::early_log("Panic in test_syscalls");
    loop {}
}

#[used]
#[link_section = ".caps"]
pub static mut CAPS: CapabilitiesRepr<4> =
    CapabilitiesRepr::new([CAP_EARLY_LOGGING, CAP_PADDING, CAP_PADDING, CAP_PADDING]);
