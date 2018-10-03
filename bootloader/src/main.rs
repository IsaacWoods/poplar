#![feature(const_fn, lang_items, ptr_internals, decl_macro)]
#![no_std]
#![no_main]

#[macro_use]
extern crate bitflags;

mod boot_services;
mod protocols;
mod runtime_services;
mod system_table;
mod types;
mod memory;

use core::panic::PanicInfo;
use crate::system_table::SystemTable;
use crate::types::Handle;

static mut SYSTEM_TABLE: *const SystemTable = 0 as *const _;

/// Returns a reference to the `SystemTable`. This is safe to call after the global has been
/// initialised, which we do straight after control is passed to us.
pub fn system_table() -> &'static SystemTable {
    unsafe { &*SYSTEM_TABLE }
}

macro print($($arg: tt)*) {
    use core::fmt::Write;
    (&*system_table().console_out).write_fmt(format_args!($($arg)*)).expect("Failed to write to console");
}

macro println {
    ($fmt: expr) => {
        print!(concat!($fmt, "\r\n"));
    },

    ($fmt: expr, $($arg: tt)*) => {
        print!(concat!($fmt, "\r\n"), $($arg)*);
    }
}

#[no_mangle]
pub extern "win64" fn uefi_main(image_handle: Handle, system_table: &'static SystemTable) -> ! {
    unsafe {
        SYSTEM_TABLE = system_table;
    }

    println!("Hello UEFI!");
    loop {}
}

#[panic_handler]
#[no_mangle]
pub fn panic(info: &PanicInfo) -> ! {
    let location = info.location().unwrap();
    println!(
        "Panic in {}({}:{})",
        location.file(),
        location.line(),
        location.column()
    );
    loop {}
}
