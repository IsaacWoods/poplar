use crate::{memory::MemoryType, uefi::Status};
use core::{ptr, slice, str};
use mer::{
    program::{ProgramHeader, SegmentType},
    Elf,
};
use x86_64::{
    boot::{ImageInfo, MemoryObjectInfo, MAX_CAPABILITY_BYTES_PER_IMAGE},
    memory::{EntryFlags, FrameSize, Size4KiB, VirtualAddress},
};

const CAPABILITY_OWNER_STR: &str = "PEBBLE";
const CAPABILITY_ENTRY_TYPE: u32 = 0;

pub fn load_image(path: &str, user_accessible: bool) -> Result<ImageInfo, Status> {
    let file_data = crate::uefi::protocols::read_file(path, crate::uefi::image_handle())?;
    let elf = Elf::new(&file_data)
        .map_err(|err| panic!("Failed to load ELF for image {}: {:?}", path, err))
        .unwrap();

    let mut image = ImageInfo::default();
    for segment in elf.segments() {
        match segment.segment_type() {
            SegmentType::Load if segment.mem_size > 0 => {
                image.add_segment(load_segment(&segment, &elf, user_accessible));
            }

            SegmentType::Note => {
                /*
                 * Search through the note entries for one containing the task's capabilities. If
                 * there is one, copy the encoded capabilities into the image info.
                 */
                let caps = segment.iterate_note_entries(&elf).unwrap().find(|entry| {
                    entry.entry_type == CAPABILITY_ENTRY_TYPE
                        && str::from_utf8(entry.name).unwrap() == CAPABILITY_OWNER_STR
                });

                if caps.is_some() {
                    let caps_length = caps.as_ref().unwrap().desc.len();
                    if caps_length > MAX_CAPABILITY_BYTES_PER_IMAGE {
                        panic!(
                            "Initial image at path {} has capability encoding of more than {} bytes!",
                            path, MAX_CAPABILITY_BYTES_PER_IMAGE
                        );
                    }

                    /*
                     * We copy at most `MAX_CAPABILITY_BYTES_PER_IMAGE` bytes from the note entry,
                     * but can safely copy less, leaving the rest of the array zero-initialized.
                     * The zero bytes are interpreted as padding by the kernel so this is fine.
                     */
                    let mut caps_array: [u8; MAX_CAPABILITY_BYTES_PER_IMAGE] = Default::default();
                    caps_array[..caps_length].copy_from_slice(&caps.unwrap().desc);
                    image.capability_stream = caps_array;
                }
            }

            _ => (),
        }
    }

    image.entry_point = VirtualAddress::new(elf.entry_point()).expect("Invalid entry point");
    Ok(image)
}

fn load_segment(segment: &ProgramHeader, elf: &Elf, user_accessible: bool) -> MemoryObjectInfo {
    assert!((segment.mem_size as usize) % Size4KiB::SIZE == 0);

    let num_frames = (segment.mem_size as usize) / Size4KiB::SIZE;
    let physical_address = crate::uefi::system_table()
        .boot_services
        .allocate_frames(MemoryType::PebbleImageMemory, num_frames)
        .map_err(|err| panic!("Failed to allocate memory for segment: {:?}", err))
        .unwrap();

    /*
     * Copy `file_size` bytes from the image into the segment's new home. Note that
     * `file_size` may be less than `mem_size`, but must never be greater than it.
     */
    assert!(segment.file_size <= segment.mem_size);
    unsafe {
        slice::from_raw_parts_mut(usize::from(physical_address) as *mut u8, segment.file_size as usize)
            .copy_from_slice(segment.data(&elf));
    }

    /*
     * Zero the remainder of the segment.
     */
    unsafe {
        ptr::write_bytes(
            (usize::from(physical_address) + (segment.file_size as usize)) as *mut u8,
            0,
            (segment.mem_size - segment.file_size) as usize,
        );
    }

    let permissions = EntryFlags::PRESENT
        | if segment.is_writable() { EntryFlags::WRITABLE } else { EntryFlags::empty() }
        | if !segment.is_executable() { EntryFlags::NO_EXECUTE } else { EntryFlags::empty() }
        | if user_accessible { EntryFlags::USER_ACCESSIBLE } else { EntryFlags::empty() };

    MemoryObjectInfo {
        physical_address,
        virtual_address: VirtualAddress::new(segment.virtual_address as usize).unwrap(),
        num_pages: num_frames,
        permissions,
    }
}
