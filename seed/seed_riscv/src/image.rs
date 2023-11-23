/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

use crate::memory::{self, MemoryManager, MemoryRegions, Region};
use core::{ptr, slice};
use hal::memory::{Flags, FrameAllocator, FrameSize, PAddr, Page, PageTable, Size4KiB, VAddr};
use hal_riscv::platform::kernel_map;
use mer::{
    program::{ProgramHeader, SegmentType},
    Elf,
};
use poplar_util::math::align_up;
use seed::boot_info::Segment;
use tracing::info;

pub fn extract_kernel(memory_regions: &mut MemoryRegions) -> Elf<'static> {
    use hal_riscv::platform::memory::RAMDISK_ADDR;

    let kernel_elf_size = unsafe { *(usize::from(RAMDISK_ADDR) as *const u32) } as usize;
    info!("Kernel elf size: {}", kernel_elf_size);

    // Reserve the kernel ELF in the memory ranges, so we don't trample over it
    memory_regions.add_region(Region::reserved(
        memory::Usage::KernelImage,
        RAMDISK_ADDR,
        align_up(kernel_elf_size + 4, Size4KiB::SIZE),
    ));

    assert_eq!(
        unsafe { &*((usize::from(RAMDISK_ADDR) + 4) as *const [u8; 4]) },
        b"\x7fELF",
        "Kernel ELF magic isn't correct"
    );
    Elf::new(unsafe { core::slice::from_raw_parts((usize::from(RAMDISK_ADDR) + 4) as *const u8, kernel_elf_size) })
        .expect("Failed to read kernel ELF :(")
}

#[derive(Clone, Debug)]
pub struct LoadedKernel {
    pub entry_point: VAddr,
    pub stack_top: VAddr,
    pub global_pointer: VAddr,

    /// The kernel is loaded to the base of the kernel address space, and then we dynamically map stuff into the
    /// space after it. This is the address of the first available page after the loaded kernel.
    pub next_available_address: VAddr,
}

pub fn load_kernel<P>(elf: Elf<'_>, page_table: &mut P, memory_manager: &MemoryManager) -> LoadedKernel
where
    P: PageTable<Size4KiB>,
{
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
