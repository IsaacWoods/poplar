#![no_std]
#![no_main]
#![feature(panic_info_message, abi_efiapi)]

mod command_line;
mod image;

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

    let fs_handle = find_root_volume_handle(&system_table).unwrap();

    if let Some(kernel_path) = command_line.kernel_path {
        // TODO: load kernel
    } else {
        panic!("No kernel path passed! What am I supposed to load?");
    }

    Status::SUCCESS
}

fn find_root_volume_handle(system_table: &SystemTable<Boot>) -> Option<Handle> {
    // Make an initial call to find how many handles we need to search
    let num_handles = system_table
        .boot_services()
        .locate_handle(SearchType::from_proto::<SimpleFileSystem>(), None)
        .expect_success("Failed to get list of filesystems");

    // Allocate a pool of the needed size
    info!("Allocating {} bytes of pool", mem::size_of::<Handle>() * num_handles);
    let pool_addr = system_table
        .boot_services()
        .allocate_pool(MemoryType::LOADER_DATA, mem::size_of::<Handle>() * num_handles)
        .expect_success("Failed to allocate pool for filesystem handles");
    let handle_slice: &mut [Handle] = unsafe { slice::from_raw_parts_mut(pool_addr as *mut Handle, num_handles) };

    // Actually fetch the handles
    system_table
        .boot_services()
        .locate_handle(SearchType::from_proto::<SimpleFileSystem>(), Some(handle_slice))
        .expect_success("Failed to get list of filesystems");

    for handle in handle_slice {
        let proto = system_table
            .boot_services()
            .handle_protocol::<SimpleFileSystem>(*handle)
            .expect_success("Failed to open SimpleFileSystem");
        // TODO: match volume label (or find a better way to find a partition)
        // proto.open_volume().expect_success("Failed to open volume").
    }

    None
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
