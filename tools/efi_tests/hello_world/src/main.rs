#![no_std]
#![no_main]
#![feature(abi_efiapi, never_type)]

use core::{fmt::Write, panic::PanicInfo};
use uefi::prelude::*;

#[entry]
fn efi_main(handle: Handle, system_table: SystemTable<Boot>) -> Status {
    write!(system_table.stdout(), "Hello, World!").unwrap();
    loop {}
}

#[panic_handler]
fn panic_handler(_info: &PanicInfo) -> ! {
    loop {}
}
