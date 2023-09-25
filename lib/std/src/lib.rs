#![allow(internal_features)]
#![feature(lang_items, prelude_import, async_iterator, core_intrinsics, panic_info_message)]
#![no_std]

extern crate alloc;

use core::panic::PanicInfo;
use linked_list_allocator::LockedHeap;

/*
 * Public re-exports. Most of this is copied from real `std`, plus our `poplar` library.
 * NOTE: deprecated re-exports, such as `std::i32` (and friends), are not included.
 */
pub use alloc::{borrow, boxed, fmt, format, rc, slice, str, string, vec};
pub use core::{
    any,
    array,
    async_iter,
    cell,
    char,
    clone,
    cmp,
    convert,
    default,
    future,
    hash,
    hint,
    intrinsics,
    iter,
    marker,
    mem,
    ops,
    option,
    pin,
    ptr,
    result,
};
pub use poplar;

// Import our own prelude for this crate
#[prelude_import]
pub use prelude::rust_2021::*;

/*
 * These modules specify the preludes that are imported in crates that depend on our fake `std`. `rustc` will use
 * the `prelude_import` attribute, like above, to import the correct prelude for the edition being built against.
 */
pub mod prelude {
    pub mod rust_2018 {
        pub use alloc::{
            boxed::Box,
            format,
            string::{String, ToString},
            vec::Vec,
        };
        pub use core::{assert_eq, panic, prelude::rust_2018::*};
    }
    pub mod rust_2021 {
        pub use alloc::{
            boxed::Box,
            format,
            string::{String, ToString},
            vec::Vec,
        };
        pub use core::{assert_eq, panic, prelude::rust_2021::*};
    }
}

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[no_mangle]
unsafe extern "C" fn _start() -> ! {
    extern "C" {
        fn main(argc: isize, argv: *const *const u8) -> isize;
    }

    // Initialize the heap
    const HEAP_START: usize = 0x600000000;
    const HEAP_SIZE: usize = 0x4000;
    let heap_memory_object =
        poplar::syscall::create_memory_object(HEAP_START, HEAP_SIZE, true, false, 0x0 as *mut usize).unwrap();
    unsafe {
        poplar::syscall::map_memory_object(&heap_memory_object, &poplar::ZERO_HANDLE, None, 0x0 as *mut usize)
            .unwrap();
        ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE);
    }

    main(0, core::ptr::null());

    poplar::syscall::early_log("Returned from main. Looping.").unwrap();
    loop {
        poplar::syscall::yield_to_kernel();
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
    use core::fmt::Write;

    // TODO: this isn't an ideal approach - if the allocator stops working we may not get a good error
    let mut s = String::new();
    if let Some(message) = info.message() {
        if let Some(location) = info.location() {
            let _ =
                write!(s, "PANIC: {} ({} - {}:{})", message, location.file(), location.line(), location.column());
        } else {
            let _ = write!(s, "PANIC: {} (no location info)", message);
        }
    }
    let _ = poplar::syscall::early_log(&s);

    loop {}
}
