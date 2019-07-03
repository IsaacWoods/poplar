use crate::{memory::MemoryType, uefi::Status};
use core::{ptr, slice};
use log::trace;
use mer::{
    program::{ProgramHeader, SegmentType},
    Elf,
};
use x86_64::{
    boot::{ImageInfo, MemoryObjectInfo},
    memory::{
        paging::{entry::EntryFlags, FRAME_SIZE},
        VirtualAddress,
    },
};

pub fn load_image(path: &str, user_accessible: bool) -> Result<ImageInfo, Status> {
    let file_data = crate::uefi::protocols::read_file(path, crate::uefi::image_handle())?;
    let elf = Elf::new(&file_data)
        .map_err(|err| panic!("Failed to load ELF for image {}: {:?}", path, err))
        .unwrap();

    let mut image = ImageInfo::default();
    for segment in elf.segments() {
        if segment.segment_type() == SegmentType::Load && segment.mem_size > 0 {
            image.add_segment(load_segment(&segment, &elf, user_accessible));
        }
    }

    image.entry_point = VirtualAddress::new(elf.entry_point()).expect("Invalid entry point");
    Ok(image)
}

fn load_segment(segment: &ProgramHeader, elf: &Elf, user_accessible: bool) -> MemoryObjectInfo {
    assert!((segment.mem_size as usize) % FRAME_SIZE == 0);

    trace!("Loading segment of size {} bytes", segment.mem_size);
    let physical_address = crate::uefi::system_table()
        .boot_services
        .allocate_frames(MemoryType::PebbleImageMemory, (segment.mem_size as usize) / FRAME_SIZE)
        .map_err(|err| panic!("Failed to allocate memory for segment: {:?}", err))
        .unwrap();

    /*
     * Copy `file_size` bytes from the image into the segment's new home. Note that
     * `file_size` may be less than `mem_size`, but must never be greater than it.
     */
    assert!(segment.file_size <= segment.mem_size);
    unsafe {
        slice::from_raw_parts_mut(
            usize::from(physical_address) as *mut u8,
            segment.file_size as usize,
        )
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
        permissions,
    }
}
