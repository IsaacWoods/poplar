#![allow(internal_features)]
#![feature(lang_items, prelude_import, async_iterator, core_intrinsics)]
#![no_std]

extern crate alloc as alloc_crate;

pub mod alloc;

/*
 * Public re-exports. Most of this is copied from real `std`, plus our `poplar` library.
 * NOTE: deprecated re-exports, such as `std::i32` (and friends), are not included.
 */
pub use alloc_crate::{borrow, boxed, collections, fmt, format, rc, slice, str, string, sync, vec};
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
    task,
};
pub use poplar;

// Import our own prelude for this crate
#[allow(unused_imports)] // Not sure why this counts as unused but the compiler thinks it is.
#[prelude_import]
pub use prelude::rust_2021::*;

/*
 * These modules specify the preludes that are imported in crates that depend on our fake `std`. `rustc` will use
 * the `prelude_import` attribute, like above, to import the correct prelude for the edition being built against.
 */
pub mod prelude {
    pub mod rust_2018 {
        pub use alloc_crate::{
            boxed::Box,
            format,
            string::{String, ToString},
            vec::Vec,
        };
        pub use core::{assert_eq, panic, prelude::rust_2018::*, todo, unreachable, write, writeln};
    }
    pub mod rust_2021 {
        pub use alloc_crate::{
            boxed::Box,
            format,
            string::{String, ToString},
            vec::Vec,
        };
        pub use core::{assert_eq, panic, prelude::rust_2021::*, todo, unreachable, write, writeln};
    }
    pub mod rust_2024 {
        pub use alloc_crate::{
            boxed::Box,
            format,
            string::{String, ToString},
            vec::Vec,
        };
        pub use core::{assert_eq, panic, prelude::rust_2024::*, todo, unreachable, write, writeln};
    }
}

use core::panic::PanicInfo;

#[cfg(target_arch = "x86_64")]
#[no_mangle]
#[unsafe(naked)]
unsafe extern "C" fn _start() -> ! {
    core::arch::naked_asm!("jmp rust_entry")
}

#[cfg(target_arch = "riscv64")]
#[no_mangle]
#[unsafe(naked)]
unsafe extern "C" fn _start() -> ! {
    core::arch::naked_asm!(
        "
        .option push
        .option norelax
        lla gp, __global_pointer$
        .option pop
        j rust_entry
        "
    )
}

#[no_mangle]
unsafe extern "C" fn rust_entry() -> ! {
    extern "C" {
        fn main(argc: isize, argv: *const *const u8) -> isize;
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

    let mut buffer = PanicBuffer::new();
    if let Some(location) = info.location() {
        let _ = write!(
            buffer,
            "PANIC: {} ({} - {}:{})",
            info.message(),
            location.file(),
            location.line(),
            location.column()
        );
    } else {
        let _ = write!(buffer, "PANIC: {} (no location info)", info.message());
    }
    let _ = poplar::syscall::early_log(buffer.as_str());

    loop {}
}

const PANIC_BUFFER_LEN: usize = 256;

pub struct PanicBuffer {
    buffer: [u8; PANIC_BUFFER_LEN],
    len: usize,
}

impl PanicBuffer {
    pub fn new() -> PanicBuffer {
        PanicBuffer { buffer: [0; PANIC_BUFFER_LEN], len: 0 }
    }

    pub fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&self.buffer[0..self.len]) }
    }
}

impl fmt::Write for PanicBuffer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        self.buffer[self.len..(self.len + bytes.len())].copy_from_slice(bytes);
        self.len += bytes.len();
        Ok(())
    }
}
