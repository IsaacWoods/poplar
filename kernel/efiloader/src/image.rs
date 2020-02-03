use uefi::{
    prelude::*,
    proto::media::{
        file::{File, FileAttribute, FileInfo, FileMode, FileType},
        fs::SimpleFileSystem,
    },
    table::boot::BootServices,
    Handle,
};

#[derive(Debug)]
pub enum ImageLoadError {
    InvalidPath,
    FailedToReadFile,
}

pub fn load_image(boot_services: &BootServices, volume_handle: Handle, path: &str) -> Result<(), ImageLoadError> {
    let mut root_file_protocol = unsafe {
        &mut *boot_services
            .handle_protocol::<SimpleFileSystem>(volume_handle)
            .expect_success("Failed to get volume")
            .get()
    }
    .open_volume()
    .expect_success("Failed to open volume");

    let mut file = root_file_protocol.open(path, FileMode::Read, FileAttribute::READ_ONLY).unwrap_success();
    let mut info_buffer = [0u8; 128];
    let info = file.get_info::<FileInfo>(&mut info_buffer).unwrap_success();
    log::info!("File info: {:?}", info.file_size());

    // TODO: allocate pool of the correct size

    match file.into_type().unwrap_success() {
        FileType::Regular(regular_file) => {
            // TODO: read data from file into pool
        }
        FileType::Dir(_) => return Err(ImageLoadError::InvalidPath),
    }

    Ok(())
}
