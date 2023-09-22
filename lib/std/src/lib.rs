#![allow(internal_features)]
#![feature(lang_items, prelude_import)]
#![no_std]

use core::panic::PanicInfo;

// Import our own prelude for this crate
#[prelude_import]
pub use prelude::rust_2021::*;

/*
 * These modules specify the preludes that are imported in crates that depend on our fake `std`. `rustc` will use
 * the `prelude_import` attribute, like above, to import the correct prelude for the edition being built against.
 */
pub mod prelude {
    pub mod rust_2018 {
        pub use core::{assert_eq, panic, prelude::rust_2018::*};
    }
    pub mod rust_2021 {
        pub use core::{assert_eq, panic, prelude::rust_2021::*};
    }
}

#[no_mangle]
unsafe extern "C" fn _start() -> ! {
    extern "C" {
        fn main(argc: isize, argv: *const *const u8) -> isize;
    }

    main(0, core::ptr::null());

    loop {
        // TODO: yield here idk
        // TODO: actually this should call an exit system call or something
    }
}

#[lang = "start"]
fn lang_start<T>(main: fn() -> T, _argc: isize, _argv: *const *const u8, _sigpipe: u8) -> isize {
    main();
    0
}

#[panic_handler]
pub fn handle_panic(info: &PanicInfo) -> ! {
    // TODO: print a panic message
    loop {}
}
