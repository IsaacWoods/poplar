#![no_std]
#![no_main]
#![feature(panic_info_message, abi_efiapi)]

mod command_line;

use command_line::CommandLine;
use core::{mem, panic::PanicInfo, slice};
use log::info;
use uefi::{
    prelude::*,
    proto::{loaded_image::LoadedImage, media::fs::SimpleFileSystem},
    table::boot::{MemoryType, SearchType},
};

const COMMAND_LINE_MAX_LENGTH: usize = 256;

static mut LOGGER: Option<uefi::logger::Logger> = None;

#[entry]
fn efi_main(image_handle: Handle, system_table: SystemTable<Boot>) -> Status {
    unsafe {
        LOGGER = Some(uefi::logger::Logger::new(system_table.stdout()));
        log::set_logger(LOGGER.as_ref().unwrap()).unwrap();
    }
    log::set_max_level(log::LevelFilter::Info);

    info!("Hello, World!");

    let loaded_image_protocol = unsafe {
        &mut *system_table
            .boot_services()
            .handle_protocol::<LoadedImage>(image_handle)
            .expect_success("Failed to open LoadedImage protocol")
            .get()
    };
    let mut buffer = [0u8; COMMAND_LINE_MAX_LENGTH];
    let load_options_str = loaded_image_protocol.load_options(&mut buffer).expect("Failed to load load options");
    let command_line = CommandLine::new(load_options_str);
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
