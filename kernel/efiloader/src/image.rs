use crate::LoaderError;
use core::{ptr, slice, str};
use hal::{
    boot_info::{LoadedImage, Segment, MAX_CAPABILITY_STREAM_LENGTH},
    memory::{Flags, FrameAllocator, FrameSize, Mapper, Page, PhysicalAddress, Size4KiB, VirtualAddress},
};
use hal_x86_64::kernel_map;
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
    table::boot::{AllocateType, BootServices, MemoryType},
    Handle,
};

pub struct KernelInfo {
    pub entry_point: usize,
    pub stack_top: usize,

    /// We load the kernel at the base of the kernel address space. We want to put other stuff after it, and so
    /// need to know how much memory the loaded image has taken up. During loading, we calculate the address of
    /// the next available page (this) to use.
    pub next_safe_address: VirtualAddress,
}

pub fn load_kernel<A, M>(
    boot_services: &BootServices,
    volume_handle: Handle,
    path: &str,
    mapper: &mut M,
    allocator: &A,
) -> Result<KernelInfo, LoaderError>
where
    A: FrameAllocator<Size4KiB>,
    M: Mapper<Size4KiB, A>,
{
    let (elf, pool_addr) = load_elf(boot_services, volume_handle, path)?;
    let entry_point = elf.entry_point();

    let mut next_safe_address = kernel_map::KERNEL_BASE;

    for segment in elf.segments() {
        match segment.segment_type() {
            SegmentType::Load if segment.mem_size > 0 => {
                let segment = load_segment(boot_services, segment, crate::KERNEL_MEMORY_TYPE, &elf, false)?;

                /*
                 * If this segment loads past `next_safe_address`, update it.
                 */
                if (segment.virtual_address + segment.size) > next_safe_address {
                    next_safe_address =
                        (Page::<Size4KiB>::contains(segment.virtual_address + segment.size) + 1).start;
                }

                assert!(segment.size % Size4KiB::SIZE == 0);
                mapper
                    .map_area(
                        segment.virtual_address,
                        segment.physical_address,
                        segment.size,
                        segment.flags,
                        allocator,
                    )
                    .unwrap();
            }

            _ => (),
        }
    }

    let stack_top = match elf.symbols().find(|symbol| symbol.name(&elf) == Some("_stack_top")) {
        Some(symbol) => symbol.value as usize,
        None => panic!("Kernel does not have a '_stack_top' symbol!"),
    };

    // Unmap the stack guard page
    let guard_page_address = match elf.symbols().find(|symbol| symbol.name(&elf) == Some("_guard_page")) {
        Some(symbol) => VirtualAddress::new(symbol.value as usize),
        None => panic!("Kernel does not have a '_guard_page' symbol!"),
    };
    assert!(guard_page_address.is_aligned(Size4KiB::SIZE));
    mapper.unmap::<Size4KiB>(Page::starts_with(guard_page_address));

    boot_services.free_pool(pool_addr).unwrap_success();
    Ok(KernelInfo { entry_point, stack_top, next_safe_address })
}

pub fn load_image(
    boot_services: &BootServices,
    volume_handle: Handle,
    name: &str,
    path: &str,
) -> Result<LoadedImage, LoaderError> {
    let (elf, pool_addr) = load_elf(boot_services, volume_handle, path)?;

    let mut image_data = LoadedImage::default();
    image_data.entry_point = VirtualAddress::new(elf.entry_point());

    let name_bytes = name.as_bytes();
    if name_bytes.len() > hal::boot_info::MAX_IMAGE_NAME_LENGTH {
        panic!("Image's name is too long: '{}'!", name);
    }
    image_data.name_length = name_bytes.len() as u8;
    (&mut image_data.name[0..name_bytes.len()]).copy_from_slice(name_bytes);

    for segment in elf.segments() {
        match segment.segment_type() {
            SegmentType::Load if segment.mem_size > 0 => {
                let segment = load_segment(boot_services, segment, crate::IMAGE_MEMORY_TYPE, &elf, true)?;
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
                    image_data.capability_stream = caps_array;
                }
            }

            _ => (),
        }
    }

    boot_services.free_pool(pool_addr).unwrap_success();
    Ok(image_data)
}

/// TODO: This returns the elf file, and also the pool addr. When the caller is done with the elf, they need to
/// free the pool themselves. When pools is made safer, we need to rework how this all works to tie the lifetime of
/// the elf to the pool.
fn load_elf<'a>(
    boot_services: &BootServices,
    volume_handle: Handle,
    path: &str,
) -> Result<(Elf<'a>, *mut u8), LoaderError> {
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

    let pool_addr = boot_services
        .allocate_pool(MemoryType::LOADER_DATA, info.file_size() as usize)
        .expect_success("Failed to allocate data for image file");
    let file_data: &mut [u8] =
        unsafe { slice::from_raw_parts_mut(pool_addr as *mut u8, info.file_size() as usize) };

    match file.into_type().unwrap_success() {
        FileType::Regular(mut regular_file) => {
            regular_file.read(file_data).expect_success("Failed to read image");
        }
        FileType::Dir(_) => return Err(LoaderError::FilePathDoesNotExist),
    }

    let elf = match Elf::new(file_data) {
        Ok(elf) => elf,
        Err(err) => panic!("Failed to load ELF for image '{}': {:?}", path, err),
    };

    Ok((elf, pool_addr))
}

fn load_segment(
    boot_services: &BootServices,
    segment: ProgramHeader,
    memory_type: MemoryType,
    elf: &Elf,
    user_accessible: bool,
) -> Result<Segment, LoaderError> {
    assert!((segment.mem_size as usize) % Size4KiB::SIZE == 0);

    let num_frames = (segment.mem_size as usize) / Size4KiB::SIZE;
    let physical_address = boot_services
        .allocate_pages(AllocateType::AnyPages, memory_type, num_frames)
        .expect_success("Failed to allocate memory for image segment!");

    /*
     * Copy `file_size` bytes from the image into the segment's new home. Note that
     * `file_size` may be less than `mem_size`, but must never be greater than it.
     */
    assert!(segment.file_size <= segment.mem_size);
    unsafe {
        slice::from_raw_parts_mut(physical_address as usize as *mut u8, segment.file_size as usize)
            .copy_from_slice(segment.data(&elf));
    }

    /*
     * Zero the remainder of the segment.
     */
    unsafe {
        ptr::write_bytes(
            ((physical_address as usize) + (segment.file_size as usize)) as *mut u8,
            0,
            (segment.mem_size - segment.file_size) as usize,
        );
    }

    Ok(Segment {
        physical_address: PhysicalAddress::new(physical_address as usize).unwrap(),
        virtual_address: VirtualAddress::new(segment.virtual_address as usize),
        size: num_frames * Size4KiB::SIZE,
        flags: Flags {
            writable: segment.is_writable(),
            executable: segment.is_executable(),
            user_accessible,
            ..Default::default()
        },
    })
}
