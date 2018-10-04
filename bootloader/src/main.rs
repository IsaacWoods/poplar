#![feature(const_fn, lang_items, ptr_internals, decl_macro)]
#![no_std]
#![no_main]

#[macro_use]
extern crate bitflags;

mod boot_services;
mod memory;
mod protocols;
mod runtime_services;
mod system_table;
mod types;

use core::panic::PanicInfo;
use crate::boot_services::{OpenProtocolAttributes, Pool, Protocol, SearchType};
use crate::protocols::{FileAttributes, FileInfo, FileMode, FileSystemInfo, SimpleFileSystem};
use crate::system_table::SystemTable;
use crate::types::{Handle, Status};

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

    let file_data = match read_file("Boot", "test.txt", image_handle) {
        Ok(data) => data,
        Err(err) => {
            println!("Failed to read file: {:?}", err);
            loop {}
        }
    };
    println!(
        "File: {}",
        core::str::from_utf8(&file_data).expect("Failed to parse file data")
    );

    loop {}
}

fn read_file(volume_label: &str, path: &str, image_handle: Handle) -> Result<Pool<[u8]>, Status> {
    let volume_root = system_table()
        .boot_services
        .locate_handle(SearchType::ByProtocol, Some(SimpleFileSystem::guid()), None)?
        .iter()
        .filter_map(|handle| {
            system_table()
                .boot_services
                .open_protocol::<SimpleFileSystem>(
                    *handle,
                    image_handle,
                    0,
                    OpenProtocolAttributes::BY_HANDLE_PROTOCOL,
                )
                .and_then(|volume| volume.open_volume())
                .ok()
        })
        .find(|root| {
            root.get_info::<FileSystemInfo>()
                .and_then(|info| info.volume_label())
                .map(|label| label == volume_label)
                .unwrap_or(false)
        })
        .ok_or(Status::NotFound)?;

    let path = boot_services::str_to_utf16(path)?;
    let file = volume_root.open(&path, FileMode::READ, FileAttributes::empty())?;

    let file_size = file.get_info::<FileInfo>()?.file_size as usize;
    let mut file_buf = system_table()
        .boot_services
        .allocate_slice::<u8>(file_size)?;

    let _ = file.read(&mut file_buf)?;
    Ok(file_buf)
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
