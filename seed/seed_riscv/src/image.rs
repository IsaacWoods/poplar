/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

use crate::{fs::File, memory::MemoryManager};
use core::{
    ptr,
    slice,
    str::{self, FromStr},
};
use hal::memory::{Flags, FrameAllocator, FrameSize, PAddr, Page, PageTable, Size4KiB, VAddr};
use hal_riscv::platform::kernel_map;
use mer::{
    program::{ProgramHeader, SegmentType},
    Elf,
};
use poplar_util::math::align_up;
use seed::boot_info::{LoadedImage, Segment, MAX_CAPABILITY_STREAM_LENGTH};

#[derive(Clone, Debug)]
pub struct LoadedKernel {
    pub entry_point: VAddr,
    pub stack_top: VAddr,
    pub global_pointer: VAddr,

    /// The kernel is loaded to the base of the kernel address space, and then we dynamically map stuff into the
    /// space after it. This is the address of the first available page after the loaded kernel.
    pub next_available_address: VAddr,
}

pub fn load_kernel<P>(file: &File<'_>, page_table: &mut P, memory_manager: &MemoryManager) -> LoadedKernel
where
    P: PageTable<Size4KiB>,
{
    let elf = Elf::new(file.data).expect("Failed to parse kernel ELF");

    let entry_point = VAddr::new(elf.entry_point());
    let mut next_available_address = kernel_map::KERNEL_BASE;

    for segment in elf.segments() {
        match segment.segment_type() {
            SegmentType::Load if segment.mem_size > 0 => {
                let segment = load_segment(segment, &elf, false, memory_manager);

                /*
                 * If this segment loads past `next_available_address`, update it.
                 */
                if (segment.virtual_address + segment.size) > next_available_address {
                    next_available_address =
                        (Page::<Size4KiB>::contains(segment.virtual_address + segment.size) + 1).start;
                }

                assert!(
                    segment.virtual_address.is_aligned(Size4KiB::SIZE),
                    "Segment's virtual address is not page-aligned"
                );
                assert!(
                    segment.physical_address.is_aligned(Size4KiB::SIZE),
                    "Segment's physical address is not frame-aligned"
                );
                assert!(segment.size % Size4KiB::SIZE == 0, "Segment size is not a multiple of page size!");
                page_table
                    .map_area(
                        segment.virtual_address,
                        segment.physical_address,
                        segment.size,
                        segment.flags,
                        memory_manager,
                    )
                    .unwrap();
            }

            _ => (),
        }
    }

    let stack_top = match elf.symbols().find(|symbol| symbol.name(&elf) == Some("_stack_top")) {
        Some(symbol) => VAddr::new(symbol.value as usize),
        None => panic!("Kernel does not have a '_stack_top' symbol!"),
    };

    let global_pointer = match elf.symbols().find(|symbol| symbol.name(&elf) == Some("__global_pointer$")) {
        Some(symbol) => VAddr::new(symbol.value as usize),
        None => panic!("Kernel does not have a '__global_pointer$' symbol!"),
    };

    // Unmap the stack guard page
    let guard_page_address = match elf.symbols().find(|symbol| symbol.name(&elf) == Some("_guard_page")) {
        Some(symbol) => VAddr::new(symbol.value as usize),
        None => panic!("Kernel does not have a '_guard_page' symbol!"),
    };
    assert!(guard_page_address.is_aligned(Size4KiB::SIZE), "Guard page address is not page aligned");
    page_table.unmap::<Size4KiB>(Page::starts_with(guard_page_address));

    LoadedKernel { entry_point, stack_top, global_pointer, next_available_address }
}

pub fn load_image(file: &File<'_>, name: &str, memory_manager: &MemoryManager) -> LoadedImage {
    let elf = Elf::new(file.data).expect("Failed to parse user task ELF");
    let mut image_data = LoadedImage::default();
    image_data.entry_point = VAddr::new(elf.entry_point());
    image_data.name = heapless::String::from_str(name).unwrap();

    for segment in elf.segments() {
        match segment.segment_type() {
            SegmentType::Load if segment.mem_size > 0 => {
                let segment = load_segment(segment, &elf, true, memory_manager);

                match image_data.segments.push(segment) {
                    Ok(()) => (),
                    Err(_) => panic!("Image for '{}' has too many load segments!", name),
                }
            }

            SegmentType::Note => {
                /*
                 * We want to search the note entries for one containing the task's capabilities (if this is an
                 * initial task). If there is one, we want to copy it into the info we pass to the kenrel.
                 */
                const CAPABILITY_OWNER_STR: &str = "POPLAR";
                const CAPABILITY_ENTRY_TYPE: u32 = 0;

                let caps = segment.iterate_note_entries(&elf).unwrap().find(|entry| {
                    entry.entry_type == CAPABILITY_ENTRY_TYPE
                        && str::from_utf8(entry.name).unwrap() == CAPABILITY_OWNER_STR
                });

                if caps.is_some() {
                    let caps_length = caps.as_ref().unwrap().desc.len();
                    if caps_length > MAX_CAPABILITY_STREAM_LENGTH {
                        panic!("Image for '{}' has too long capability encoding!", name);
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

    image_data
}

fn load_segment(
    segment: ProgramHeader,
    elf: &Elf,
    user_accessible: bool,
    memory_manager: &MemoryManager,
) -> Segment {
    /*
     * We don't require each segment to fill up all the pages it needs - as long as the start of each segment is
     * page-aligned so they don't overlap, it's fine. This is mainly to support images linked by `lld` with the `-z
     * separate-loadable-segments` flag, which does this, and also so TLS segments don't fill up more space than
     * they need (so the kernel knows its actual size, and can align that to a page if it needs to).
     *
     * However, we do need to align up to the page margin here so we zero all the memory allocated.
     */
    let mem_size = align_up(segment.mem_size as usize, Size4KiB::SIZE);

    let num_frames = (mem_size as usize) / Size4KiB::SIZE;
    let physical_address = memory_manager.allocate_n(num_frames).start.start;

    /*
     * Copy `file_size` bytes from the image into the segment's new home. Note that
     * `file_size` may be less than `mem_size`, but must never be greater than it.
     * NOTE: we use the segment's memory size here, before we align it up to the page margin.
     */
    assert!(segment.file_size <= segment.mem_size, "Segment's data will not fit in requested memory");
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
            mem_size - (segment.file_size as usize),
        );
    }

    Segment {
        physical_address: PAddr::new(usize::from(physical_address)).unwrap(),
        virtual_address: VAddr::new(segment.virtual_address as usize),
        size: num_frames * Size4KiB::SIZE,
        flags: Flags {
            writable: segment.is_writable(),
            executable: segment.is_executable(),
            user_accessible,
            ..Default::default()
        },
    }
}
