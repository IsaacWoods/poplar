use boot_info::{LoadedImage, Segment, MAX_CAPABILITY_STREAM_LENGTH};
use core::{slice, str};
use mer::{
    program::{ProgramHeader, SegmentType},
    Elf,
};
use uefi::{
    prelude::*,
    proto::media::{
        file::{File, FileAttribute, FileInfo, FileMode, FileType},
        fs::SimpleFileSystem,
    },
    table::boot::{BootServices, MemoryType},
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

    let pool_addr = boot_services
        .allocate_pool(MemoryType::LOADER_DATA, info.file_size() as usize)
        .expect_success("Failed to allocate data for image file");
    let file_data: &mut [u8] =
        unsafe { slice::from_raw_parts_mut(pool_addr as *mut u8, info.file_size() as usize) };

    match file.into_type().unwrap_success() {
        FileType::Regular(mut regular_file) => {
            regular_file.read(file_data).expect_success("Failed to read image");
        }
        FileType::Dir(_) => return Err(ImageLoadError::InvalidPath),
    }

    let elf = match Elf::new(file_data) {
        Ok(elf) => elf,
        Err(err) => panic!("Failed to load ELF for image '{}': {:?}", path, err),
    };

    let mut image_data = LoadedImage::default();
    image_data.entry_point = elf.entry_point();

    for segment in elf.segments() {
        match segment.segment_type() {
            SegmentType::Load if segment.mem_size > 0 => {
                let segment = load_segment(boot_services, segment)?;
                match image_data.add_segment(segment) {
                    Ok(()) => (),
                    Err(()) => panic!("Image at '{}' has too many load segments!", path),
                }
            }

            SegmentType::Note => {
                /*
                 * We want to search the note entries for one containing the task's capabilities (if this is an
                 * initial task). If there is one, we want to copy it into the info we pass to the kenrel.
                 */
                const CAPABILITY_OWNER_STR: &str = "PEBBLE";
                const CAPABILITY_ENTRY_TYPE: u32 = 0;

                let caps = segment.iterate_note_entries(&elf).unwrap().find(|entry| {
                    entry.entry_type == CAPABILITY_ENTRY_TYPE
                        && str::from_utf8(entry.name).unwrap() == CAPABILITY_OWNER_STR
                });

                if caps.is_some() {
                    let caps_length = caps.as_ref().unwrap().desc.len();
                    if caps_length > MAX_CAPABILITY_STREAM_LENGTH {
                        panic!("Image at path '{}' has too long capability encoding!", path);
                    }

                    /*
                     * We copy at most `MAX_CAPABILITY_BYTES_PER_IMAGE` bytes from the note entry,
                     * but can safely copy less, leaving the rest of the array zero-initialized.
                     * The zero bytes are interpreted as padding by the kernel so this is fine.
                     */
                    let mut caps_array: [u8; MAX_CAPABILITY_STREAM_LENGTH] = Default::default();
                    caps_array[..caps_length].copy_from_slice(&caps.unwrap().desc);
                    image.capability_stream = caps_array;
                }
            }

            _ => (),
        }
    }

    Ok(())
}

fn load_segment(
    boot_services: &BootServices,
    segment: ProgramHeader,
    elf: &Elf,
    user_accessible: bool,
) -> Result<boot_info::Segment, ImageLoadError> {
    unimplemented!()
}
