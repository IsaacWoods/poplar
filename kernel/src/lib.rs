#![no_std]
#![feature(asm)]

/*
 * This selects the correct module to include depending on the architecture we're compiling the
 * kernel for. These architecture modules contain the kernel entry point and any platform-specific
 * code.
 */
cfg_if! {
    if #[cfg(feature = "x86_64")] {
        mod x86_64;
        pub use crate::x86_64::kmain;
    } else {
        compile_error!("Tried to build kernel without specifying an architecture!");
    }
}

use cfg_if::cfg_if;
use core::panic::PanicInfo;

#[panic_handler]
#[no_mangle]
pub extern "C" fn panic(info: &PanicInfo) -> ! {
    // TODO
    loop {}
}
