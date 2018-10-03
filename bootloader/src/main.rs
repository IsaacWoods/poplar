#![feature(try_trait, uniform_paths, const_raw_ptr_deref, decl_macro)]
#![no_std]
#![no_main]

mod uefi;
#[macro_use]
mod text;
mod boot;
mod memory;
mod runtime;

use boot::BootServices;
use core::panic::PanicInfo;
use memory::MemoryDescriptor;
use runtime::RuntimeServices;
use text::{TextInput, TextOutput};
use uefi::{Guid, Handle, UefiStatus};

/// This is a wrapper to access the system table from the mutable static. It evaluates to an
/// expression of type `&mut SystemTable`.
macro system_table() {
    unsafe { &mut *crate::SYSTEM_TABLE }
}

pub static mut SYSTEM_TABLE: *mut SystemTable = 0x0 as *mut SystemTable;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TableHeader {
    signature: u64,
    revision: u32,
    header_size: u32,
    crc32: u32,
    _reserved: u32,
}

#[repr(C)]
pub struct ConfigurationTable {
    pub vendor_guid: Guid,
    pub vendor_table: usize,
}

#[repr(C)]
pub struct SystemTable {
    pub header: TableHeader,
    pub firmware_vendor: *const u16,
    pub firmware_revision: u32,
    pub console_in_handle: Handle,
    pub console_in: &'static mut TextInput,
    pub console_out_handle: Handle,
    pub console_out: &'static mut TextOutput,
    pub console_error_handle: Handle,
    pub console_error: &'static mut TextOutput,
    pub runtime_services: &'static mut RuntimeServices,
    pub boot_services: &'static mut BootServices,
    pub total_table_entries: usize,
    pub configuration_tables: *const ConfigurationTable,
}

#[no_mangle]
pub extern "win64" fn uefi_main(
    image_handle: Handle,
    system_table: &'static mut SystemTable,
) -> UefiStatus {
    unsafe {
        SYSTEM_TABLE = system_table as *mut SystemTable;
    }

    println!("Hello from Rust UEFI land!!!");
    UefiStatus::Success
}

#[panic_handler]
#[no_mangle]
pub fn rust_panic(info: &PanicInfo) -> ! {
    let location = info.location().unwrap();
    println!(
        "Panic in {}({}:{})",
        location.file(),
        location.line(),
        location.column()
    );
    loop {}
}
