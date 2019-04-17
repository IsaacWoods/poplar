use crate::{
    memory::{BootFrameAllocator, MemoryType},
    types::Status,
};
use core::slice;
use log::info;
use mer::{
    section::{SectionHeader, SectionType},
    Elf,
};
use x86_64::memory::{
    paging::{entry::EntryFlags, table::IdentityMapping, Frame, Mapper, Page, FRAME_SIZE},
    PhysicalAddress,
    VirtualAddress,
};

pub struct ImageInfo<'a> {
    pub physical_base: PhysicalAddress,
    pub elf: Elf<'a>,
}

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
    mapper: &mut Mapper<IdentityMapping>,
    allocator: &BootFrameAllocator,
    user_accessible: bool,
) -> Result<ImageInfo<'a>, Status> {
    let elf = Elf::new(&image_data)
        .map_err(|err| panic!("Failed to parse ELF({}): {:?}", path, err))
        .unwrap();

    /*
     * Work out how much space we need and check it's a multiple of the page size.
     */
    let image_size =
        elf.sections().fold(
            0,
            |size, section| {
                if section.is_allocated() {
                    size + section.size
                } else {
                    size
                }
            },
        ) as usize;

    if image_size % FRAME_SIZE != 0 {
        panic!("Image size is not a multiple of the frame size: {}", path);
    }

    /*
     * Allocate enough memory and zero it.
     */
    let physical_base = crate::system_table()
        .boot_services
        .allocate_frames(memory_type, image_size / FRAME_SIZE)
        .map_err(|err| panic!("Failed to allocate memory for image({}): {:?}", path, err))
        .unwrap();

    unsafe {
        crate::system_table().boot_services.set_mem(
            usize::from(physical_base) as *mut _,
            image_size as usize,
            0,
        );
    }

    /*
     * Load the sections of the ELF into memory, after which we can free the ELF. We use sections
     * instead of segments because it allows us to define permissions on a per-section basis.
     */
    let mut section_physical_address = physical_base;

    for section in elf.sections() {
        // Skip sections that shouln't be loaded or ones with no data
        if !section.is_allocated() || section.size == 0 {
            continue;
        }

        info!(
            "Loading section of '{}': '{}' at {:#x}-{:#x} at physical address {:#x}",
            path,
            section.name(&elf).unwrap(),
            section.address,
            section.address + section.size - 1,
            section_physical_address,
        );

        map_section(mapper, section_physical_address, &section, allocator, user_accessible);

        /*
         * For `ProgBits` sections, we copy the data from the image into the section's new home.
         * For `NoBits` sections, we leave it zeroed.
         */
        if let SectionType::ProgBits = section.section_type() {
            unsafe {
                slice::from_raw_parts_mut(
                    usize::from(section_physical_address) as *mut u8,
                    section.size as usize,
                )
                .copy_from_slice(section.data(&elf).unwrap());
            }
        }

        section_physical_address += section.size as usize;
    }

    Ok(ImageInfo { physical_base, elf })
}

fn map_section(
    mapper: &mut Mapper<IdentityMapping>,
    physical_base: PhysicalAddress,
    section: &SectionHeader,
    allocator: &BootFrameAllocator,
    user_accessible: bool,
) {
    let virtual_address = VirtualAddress::new(section.address as usize).unwrap();
    /*
     * Because the addresses should be page-aligned, the half-open ranges `[physical_base,
     * physical_base + size)` and `[virtual_address, virtual_address + size)` gives us the
     * correct frame and page ranges.
     */
    let frames =
        Frame::contains(physical_base)..Frame::contains(physical_base + section.size as usize);
    let pages =
        Page::contains(virtual_address)..Page::contains(virtual_address + section.size as usize);
    assert!(frames.clone().count() == pages.clone().count());

    /*
     * Work out the most restrictive set of permissions this section can be mapped with. If the
     * section needs to be writable, mark the pages as writable. If the section does **not**
     * contain executable instructions, mark it as `NO_EXECUTE`.
     */
    let flags = EntryFlags::PRESENT
        | if section.is_writable() { EntryFlags::WRITABLE } else { EntryFlags::empty() }
        | if !section.is_executable() { EntryFlags::NO_EXECUTE } else { EntryFlags::empty() }
        | if user_accessible { EntryFlags::USER_ACCESSIBLE } else { EntryFlags::empty() };

    for (frame, page) in frames.zip(pages) {
        mapper.map_to(page, frame, flags, allocator);
    }
}
