use crate::{
    memory::{BootFrameAllocator, MemoryType},
    uefi::Status,
};
use core::{ptr, slice};
use log::info;
use mer::{
    program::{ProgramHeader, SegmentType},
    Elf,
};
use x86_64::memory::{EntryFlags, Frame, FrameSize, Mapper, Page, PhysicalAddress, Size4KiB, VirtualAddress};

/// Loads an ELF from the given path on the boot volume, allocates physical memory for it, and
/// copies its sections into the new memory. Also maps each allocated section into the given set of
/// page tables.
///
/// This borrows the file data, instead of reading the file itself, so that it can return the
/// loaded `Elf` back to the caller.
pub fn load_image<'a>(
    path: &str,
    image_data: &'a [u8],
    memory_type: MemoryType,
    mapper: &mut Mapper,
    allocator: &BootFrameAllocator,
    user_accessible: bool,
) -> Result<Elf<'a>, Status> {
    let elf = Elf::new(&image_data).map_err(|err| panic!("Failed to parse ELF({}): {:?}", path, err)).unwrap();

    /*
     * Load each segment into memory, after which we can free the ELF. We don't map the segments
     * are the specified physical addresses, because they don't matter.
     */
    for segment in elf.segments() {
        if segment.segment_type() == SegmentType::Load {
            info!(
                "Mapping Load segment at virtual address {:#x} with flags: {:#b}",
                segment.virtual_address, segment.flags
            );
            assert!((segment.mem_size as usize) % Size4KiB::SIZE == 0);

            let physical_address = crate::uefi::system_table()
                .boot_services
                .allocate_frames(memory_type, (segment.mem_size as usize) / Size4KiB::SIZE)
                .map_err(|err| panic!("Failed to allocate memory for image({}): {:?}", path, err))
                .unwrap();

            map_segment(&segment, physical_address, user_accessible, mapper, allocator);

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
        }
    }

    Ok(elf)
}

fn map_segment(
    segment: &ProgramHeader,
    physical_address: PhysicalAddress,
    user_accessible: bool,
    mapper: &mut Mapper,
    allocator: &BootFrameAllocator,
) {
    let virtual_address = VirtualAddress::new(segment.virtual_address as usize).unwrap();
    let flags = EntryFlags::PRESENT
        | if segment.is_writable() { EntryFlags::WRITABLE } else { EntryFlags::empty() }
        | if !segment.is_executable() { EntryFlags::NO_EXECUTE } else { EntryFlags::empty() }
        | if user_accessible { EntryFlags::USER_ACCESSIBLE } else { EntryFlags::empty() };
    let frames = Frame::contains(physical_address)..Frame::contains(physical_address + segment.mem_size as usize);
    let pages = Page::contains(virtual_address)..Page::contains(virtual_address + segment.mem_size as usize);
    assert!(frames.clone().count() == pages.clone().count());

    for (frame, page) in frames.zip(pages) {
        mapper.map_to(page, frame, flags, allocator).unwrap();
    }
}
