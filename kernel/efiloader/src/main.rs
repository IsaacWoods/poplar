#![no_std]
#![no_main]
#![feature(panic_info_message, abi_efiapi)]

mod command_line;
mod image;

use command_line::CommandLine;
use core::{mem, panic::PanicInfo, slice};
use log::{error, info};
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

    // TODO: return upon error instead of panicking
    let fs_handle = find_volume(&system_table, command_line.volume_label.expect("No volume label supplied"))
        .expect("No disk with the given volume label");

    let kernel_info = if let Some(kernel_path) = command_line.kernel_path {
        match image::load_image(system_table.boot_services(), fs_handle, kernel_path) {
            Ok(kernel_info) => kernel_info,
            Err(err) => {
                error!("Failed to load kernel: {:?}", err);
                return Status::LOAD_ERROR;
            }
        }
    } else {
        error!("No kernel path passed! What am I supposed to load?");
        return Status::INVALID_PARAMETER;
    };

    Status::SUCCESS
}

fn find_volume(system_table: &SystemTable<Boot>, label: &str) -> Option<Handle> {
    use uefi::proto::media::file::{File, FileSystemVolumeLabel};

    // Make an initial call to find how many handles we need to search
    let num_handles = system_table
        .boot_services()
        .locate_handle(SearchType::from_proto::<SimpleFileSystem>(), None)
        .expect_success("Failed to get list of filesystems");

    // Allocate a pool of the needed size
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

    // TODO: the `&mut` here is load-bearing, because we free the pool, and so need to copy the handle for if we
    // want to return it, otherwise it disappears out from under us. This should probably be rewritten to not work
    // like that. We could use a `Pool` type that manages the allocation and is automatically freed when dropped.
    for &mut handle in handle_slice {
        let proto = unsafe {
            &mut *system_table
                .boot_services()
                .handle_protocol::<SimpleFileSystem>(handle)
                .expect_success("Failed to open SimpleFileSystem")
                .get()
        };
        let mut buffer = [0u8; 32];
        let volume_label = proto
            .open_volume()
            .expect_success("Failed to open volume")
            .get_info::<FileSystemVolumeLabel>(&mut buffer)
            .expect_success("Failed to get volume label")
            // TODO: maybe change uefi to take a buffer here and return a &str (allows us to remove dependency on
            // ucs2 here for one)
            .volume_label();

        let mut str_buffer = [0u8; 32];
        let length = ucs2::decode(volume_label.to_u16_slice(), &mut str_buffer).unwrap();
        let volume_label_str = core::str::from_utf8(&str_buffer[0..length]).unwrap();

        if volume_label_str == label {
            system_table.boot_services().free_pool(pool_addr).unwrap_success();
            return Some(handle);
        }
    }

    system_table.boot_services().free_pool(pool_addr).unwrap_success();
    None
}

#[panic_handler]
fn panic_handler(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        error!("Panic in {} at ({}:{})", location.file(), location.line(), location.column());
        if let Some(message) = info.message() {
            error!("Panic message: {}", message);
        }
    }
    loop {}
}
