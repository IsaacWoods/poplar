//! This module contains the physical memory manager Pebble uses on x86_64.

mod buddy_allocator;
pub mod physical;
pub mod userspace_map;

use self::physical::LockedPhysicalMemoryManager;
use crate::util::bitmap::Bitmap;
use alloc::collections::BTreeMap;
use bit_field::BitField;
use x86_64::memory::{
    kernel_map,
    paging::{
        entry::EntryFlags,
        table::RecursiveMapping,
        ActivePageTable,
        Frame,
        Page,
        FRAME_SIZE,
        PAGE_SIZE,
    },
    PhysicalAddress,
    VirtualAddress,
};

/// Type alias to hide the concrete type of the kernel's page tables, as most users won't care
/// about the specifics.
pub type KernelPageTable = ActivePageTable<RecursiveMapping>;

/// Sometimes the system needs to access specific areas of physical memory. For example,
/// memory-mapped hardware or configuration tables are located at specific physical addresses.
/// `PhysicalMapping`s provide an easy way to map a given physical address to a virtual address in
/// the kernel address space, if you don't care what address it ends up at. This is perfect for,
/// for example, ACPI or the APIC driver.
#[derive(Clone, Copy, Debug)]
pub struct PhysicalMapping {
    /// The address of the start of the mapping in the physical address space.
    pub physical_base: PhysicalAddress,

    /// The address of the start of the mapping in the virtual address space.
    pub virtual_base: VirtualAddress,

    /// Size, in bytes, of the mapping. Must be a multiple of the page size.
    pub size: usize,
}

pub struct PhysicalRegionMapper {
    /// This maps `PhysicalMapping`s to their starting `PhysicalAddress`s.
    pub mappings: BTreeMap<PhysicalAddress, PhysicalMapping>,

    /// This tracks which of the pages in the area of virtual memory we map `PhysicalMapping`s into
    /// is free (where 0 = free, 1 = used). There are 32 pages in the area, so we need 32 bits.
    /// The `crate::util::bitmap::Bitmap` trait makes it easy to use this as a bitmap.
    pub virtual_area_bitmap: u32,
}

impl PhysicalRegionMapper {
    pub fn new() -> PhysicalRegionMapper {
        PhysicalRegionMapper { mappings: BTreeMap::new(), virtual_area_bitmap: 0 }
    }

    pub fn map_physical_region(
        &mut self,
        start_frame: Frame,
        number_of_frames: usize,
        flags: EntryFlags,
        page_tables: &mut KernelPageTable,
        frame_allocator: &LockedPhysicalMemoryManager,
    ) -> PhysicalMapping {
        let virtual_region_start = self
            .virtual_area_bitmap
            .alloc(number_of_frames)
            .expect("Not enough space for physical mapping");
        let frames = start_frame..(start_frame + number_of_frames);
        let start_page =
            Page::contains(kernel_map::PHYSICAL_MAPPING_START) + (virtual_region_start as usize);
        let pages = start_page..(start_page + number_of_frames);

        for (frame, page) in frames.zip(pages) {
            page_tables.map_to(page, frame, EntryFlags::PRESENT | flags, frame_allocator);
        }

        let mapping = PhysicalMapping {
            physical_base: start_frame.start_address(),
            virtual_base: start_page.start_address(),
            size: number_of_frames * FRAME_SIZE,
        };

        self.mappings.insert(start_frame.start_address(), mapping);
        mapping
    }

    pub fn unmap_physical_region(
        &mut self,
        mapping: PhysicalMapping,
        page_tables: &mut KernelPageTable,
        frame_allocator: &LockedPhysicalMemoryManager,
    ) {
        for page in Page::contains(mapping.virtual_base)
            ..Page::contains(mapping.virtual_base + mapping.size)
        {
            // Unmap it from the virtual address space
            page_tables.unmap(page, frame_allocator);

            // Free it in the bitmap so the page can be used by a future physical mapping
            self.virtual_area_bitmap.set_bit(
                (usize::from(page.start_address())
                    - usize::from(kernel_map::PHYSICAL_MAPPING_START))
                    / PAGE_SIZE,
                false,
            );
        }
    }
}
