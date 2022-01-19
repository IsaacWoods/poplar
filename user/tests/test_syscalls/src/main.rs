#![no_std]
#![no_main]

use core::panic::PanicInfo;
use poplar::{
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

    for i in 0..10 {
        let a = 0;
        let b = 1;
        let c = 2;
        let d = 3;
        let e = 4;
        let result = unsafe { syscall::raw::syscall5(syscall::SYSCALL_TEST, a, b, c, d, e) };
        assert_eq!(a, 0);
        assert_eq!(b, 1);
        assert_eq!(c, 2);
        assert_eq!(d, 3);
        assert_eq!(e, 4);
        assert_eq!(result, 963);
    }

    loop {
        // syscall::early_log("Yielding").unwrap();
        // syscall::yield_to_kernel();
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
