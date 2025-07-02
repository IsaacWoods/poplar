/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

use crate::{fs::File, memory::MemoryManager};
use core::{ptr, slice};
use hal::memory::{Flags, FrameAllocator, FrameSize, PAddr, Page, PageTable, Size4KiB, VAddr};
use mer::{
    program::{ProgramHeader, SegmentType},
    Elf,
};
use mulch::math::align_up;
use seed_bootinfo::{LoadedSegment, SegmentFlags};

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
    let mut next_available_address = crate::kernel_map::KERNEL_IMAGE_BASE;

    for segment in elf.segments() {
        match segment.segment_type() {
            SegmentType::Load if segment.mem_size > 0 => {
                let segment = load_segment(segment, &elf, false, memory_manager);
                let phys_addr = PAddr::new(segment.phys_addr as usize).unwrap();
                let virt_addr = VAddr::new(segment.virt_addr as usize);
                let size = segment.size as usize;

                /*
                 * If this segment loads past `next_available_address`, update it.
                 */
                if (virt_addr + size) > next_available_address {
                    next_available_address = (Page::<Size4KiB>::contains(virt_addr + size) + 1).start;
                }

                assert!(virt_addr.is_aligned(Size4KiB::SIZE), "Segment's virtual address is not page-aligned");
                assert!(phys_addr.is_aligned(Size4KiB::SIZE), "Segment's physical address is not frame-aligned");
                assert!(size % Size4KiB::SIZE == 0, "Segment size is not a multiple of page size!");
                page_table
                    .map_area(
                        virt_addr,
                        phys_addr,
                        size,
                        Flags {
                            writable: segment.flags.get(SegmentFlags::WRITABLE),
                            executable: segment.flags.get(SegmentFlags::EXECUTABLE),
                            user_accessible: false,
                            cached: true,
                        },
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

pub struct LoadedImageInfo {
    pub entry_point: VAddr,
    pub num_segments: u16,
    pub segments: [seed_bootinfo::LoadedSegment; seed_bootinfo::LOADED_IMAGE_MAX_SEGMENTS],
}

pub fn load_image(file: &File<'_>, name: &str, memory_manager: &MemoryManager) -> LoadedImageInfo {
    let elf = Elf::new(file.data).expect("Failed to parse user task ELF");

    let entry_point = VAddr::new(elf.entry_point());
    let mut segments = [seed_bootinfo::LoadedSegment::default(); seed_bootinfo::LOADED_IMAGE_MAX_SEGMENTS];
    let mut num_segments = 0;
    for segment in elf.segments() {
        match segment.segment_type() {
            SegmentType::Load if segment.mem_size > 0 => {
                if (num_segments + 1) > seed_bootinfo::LOADED_IMAGE_MAX_SEGMENTS {
                    panic!("Loaded image '{}' has too many loaded segments!", name);
                }

                let segment = load_segment(segment, &elf, true, memory_manager);
                segments[num_segments] = segment;
                num_segments += 1;
            }
            _ => (),
        }
    }

    LoadedImageInfo { entry_point, num_segments: num_segments as u16, segments }
}

fn load_segment(
    segment: ProgramHeader,
    elf: &Elf,
    user_accessible: bool,
    memory_manager: &MemoryManager,
) -> LoadedSegment {
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

    let flags = SegmentFlags::new()
        .with(SegmentFlags::WRITABLE, segment.is_writable())
        .with(SegmentFlags::EXECUTABLE, segment.is_executable());
    LoadedSegment {
        phys_addr: usize::from(physical_address) as u64,
        virt_addr: segment.virtual_address,
        size: (num_frames * Size4KiB::SIZE) as u32,
        flags,
    }
}
