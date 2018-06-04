/*
 * Copyright (C) 2017, Pebble Developers.
 * See LICENCE.md
 */

use core::intrinsics;
use core::panic::PanicInfo;
use cpu;

#[lang = "eh_personality"]
#[no_mangle]
pub extern "C" fn rust_eh_personality() {}

#[panic_implementation]
#[no_mangle]
pub extern "C" fn panic(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        error!(
            "PANIC in {} at line {}: \n    {}",
            location.file(),
            location.line(),
            info.message().unwrap()
        );
    } else {
        error!("PANIC at ???: \n    {}", info.message().unwrap());
    }

    loop {
        unsafe {
            cpu::halt();
        }
    }
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn _Unwind_Resume() {
    loop {
        unsafe {
            cpu::halt();
        }
    }
}
