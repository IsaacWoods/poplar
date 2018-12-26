//! This module contains the physical memory manager Pebble uses on x86_64.

mod buddy_allocator;
pub mod physical;

use self::physical::LockedPhysicalMemoryManager;
use crate::util::bitmap::Bitmap;
use alloc::collections::BTreeMap;
use x86_64::memory::paging::entry::EntryFlags;
use x86_64::memory::paging::table::RecursiveMapping;
use x86_64::memory::paging::{ActivePageTable, Frame, Page, FRAME_SIZE};
use x86_64::memory::{kernel_map, PhysicalAddress, VirtualAddress};

/// Type alias to hide the concrete type of the kernel's page tables, as most users won't care
/// about the specifics.
pub type KernelPageTable = ActivePageTable<RecursiveMapping>;

/// Sometimes the system needs to access specific areas of physical memory. For example,
/// memory-mapped hardware or configuration tables are located at specific physical addresses.
/// `PhysicalMapping`s provide an easy way to map a given physical address to a virtual address in
/// the kernel address space, if you don't care what address it ends up at. This is perfect for,
/// for example, ACPI or the APIC driver.
#[derive(Clone, Copy)]
pub struct PhysicalMapping {
    /// The address of the start of the mapping in the physical address space.
    physical_base: PhysicalAddress,

    /// The address of the start of the mapping in the virtual address space.
    virtual_base: VirtualAddress,

    /// Size, in bytes, of the mapping. Must be a multiple of the page size.
    size: usize,
}

pub struct PhysicalRegionMapper {
    /// This maps `PhysicalMapping`s to their starting `PhysicalAddress`s.
    pub mappings: BTreeMap<PhysicalAddress, PhysicalMapping>,

    /// This tracks which of the pages in the area of virtual memory we map `PhysicalMapping`s into
    /// is free (0 = free, 1 = used). There are 32 pages in the area, so we need 32 bits.
    pub virtual_area_bitmap: Bitmap<u32>,
}

impl PhysicalRegionMapper {
    pub fn new() -> PhysicalRegionMapper {
        PhysicalRegionMapper {
            mappings: BTreeMap::new(),
            virtual_area_bitmap: Bitmap::new(0),
        }
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
            .alloc_n(number_of_frames)
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

    pub fn unmap_physical_region(&mut self, mapping: PhysicalMapping) {
        // TODO
        unimplemented!();
    }
}
