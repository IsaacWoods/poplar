#![no_std]
#![no_main]
#![feature(panic_info_message, abi_efiapi)]

use core::panic::PanicInfo;
use log::info;
use uefi::prelude::*;

static mut LOGGER: Option<uefi::logger::Logger> = None;

#[entry]
fn efi_main(image: Handle, system_table: SystemTable<Boot>) -> Status {
    unsafe {
        LOGGER = Some(uefi::logger::Logger::new(system_table.stdout()));
        log::set_logger(LOGGER.as_ref().unwrap()).unwrap();
    }
    log::set_max_level(log::LevelFilter::Info);

    info!("Hello, World!");
    Status::SUCCESS
}

#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    use log::error;

    if let Some(location) = info.location() {
        error!("Panic in {} at ({}:{})", location.file(), location.line(), location.column());
        if let Some(message) = info.message() {
            error!("Panic message: {}", message);
        }
    }
    loop {}
}
