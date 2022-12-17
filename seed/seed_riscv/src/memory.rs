/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

use arrayvec::ArrayVec;
use core::{fmt, ops::Range, ptr::NonNull};
use hal::memory::{Frame, FrameAllocator, FrameSize, PAddr, Size4KiB};
use poplar_util::ranges::RangeIntersect;
use spinning_top::Spinlock;
use tracing::trace;

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum RegionType {
    #[default]
    Usable,
    Reserved(Usage),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Usage {
    Firmware,
    DeviceTree,
    Seed,
    KernelImage,
    Unknown,
}

#[derive(Clone, Copy, Default)]
pub struct Region {
    pub typ: RegionType,
    pub address: PAddr,
    pub size: usize,
}

impl Region {
    pub fn new(typ: RegionType, address: PAddr, size: usize) -> Region {
        assert_eq!(size % Size4KiB::SIZE, 0);
        Region { typ, address, size }
    }

    pub fn usable(address: PAddr, size: usize) -> Region {
        Self::new(RegionType::Usable, address, size)
    }

    pub fn reserved(usage: Usage, address: PAddr, size: usize) -> Region {
        Self::new(RegionType::Reserved(usage), address, size)
    }

    pub fn range(&self) -> Range<PAddr> {
        self.address..(self.address + self.size)
    }
}

impl fmt::Debug for Region {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Region({:?}, {:#x}..{:#x})", self.typ, self.address, self.address + self.size)
    }
}

const MAX_REGIONS: usize = 32;

/// The region map provides a high-level view of the physical memory space, containing large regions of memory that
/// are either usable, or reserved for one of a variety of reasons. This information is static: we don't allocate
/// out of the regions directly - a physical memory allocator is provided by `MemoryManager`.
#[derive(Clone, Debug)]
pub struct MemoryRegions(ArrayVec<Region, MAX_REGIONS>);

impl MemoryRegions {
    pub fn new() -> MemoryRegions {
        MemoryRegions(ArrayVec::new())
    }

    /// Add a region of memory to the manager, merging and handling intersecting regions as needed.
    pub fn add_region(&mut self, region: Region) {
        let mut added = false;

        for mut existing in &mut self.0 {
            if region.typ == existing.typ {
                /*
                 * The new region is the same type as the existing region - see if the new region is contained
                 * inside the existing one, or if we we can merge it onto the front or end.
                 * TODO: this doesn't consider the case of a new region connecting two regions so that all three
                 * can be merged - do we care?
                 */
                if existing.range().encompasses(region.range()) {
                    added = true;
                } else if (region.address + region.size) == existing.address {
                    existing.address = region.address;
                    existing.size += region.size;
                    added = true;
                } else if (existing.address + existing.size) == region.address {
                    existing.size += region.size;
                    added = true;
                }
            }
        }

        /*
         * We now consider regions that aren't the same type as the region we're adding; if this new region covers
         * part of an existing region, we need to 'shrink' that region so that they don't overlap. If the new
         * region is in the middle of another, we have to split the existing region into two.
         */
        self.0 = self
            .0
            .clone()
            .into_iter()
            .flat_map(|existing| {
                let mut new_entries = ArrayVec::<Region, 2>::new();

                if existing.typ == region.typ {
                    new_entries.push(existing);
                    return new_entries;
                }

                let (before, middle, after) = existing.range().split(region.range());
                if middle.is_none() {
                    // The regions don't intersect - add the existing region back
                    new_entries.push(existing);
                } else {
                    /*
                     * The regions do intersect, and so we need to remove the portion that intersects. Add the
                     * portions that don't intersect the new region (potentially one before and one after) back as
                     * two separate regions.
                     */
                    if let Some(before) = before {
                        new_entries.push(Region::new(
                            existing.typ,
                            before.start,
                            usize::from(before.end) - usize::from(before.start),
                        ));
                    }
                    if let Some(after) = after {
                        new_entries.push(Region::new(
                            existing.typ,
                            after.start,
                            usize::from(after.end) - usize::from(after.start),
                        ));
                    }
                }
                new_entries
            })
            .collect();

        if !added {
            self.0.push(region);
        }
    }
}

/// The physical memory manager - this consumes a `MemoryRegions` map, and uses it to initialise an
/// instrusive free list of all usable physical memory. This can then be used to allocate physical memory
/// as needed, at frame granularity.
pub struct MemoryManager(Spinlock<MemoryManagerInner>);

unsafe impl Send for MemoryManager {}
unsafe impl Sync for MemoryManager {}

pub struct MemoryManagerInner {
    regions: Option<MemoryRegions>,
    usable_head: Option<NonNull<Node>>,
    usable_tail: Option<NonNull<Node>>,
}

/// Memory nodes are stored intrusively in the memory that they manage.
#[derive(Clone, Copy, Debug)]
pub struct Node {
    size: usize,
    prev: Option<NonNull<Node>>,
    next: Option<NonNull<Node>>,
}

impl MemoryManager {
    pub const fn new() -> MemoryManager {
        MemoryManager(Spinlock::new(MemoryManagerInner { regions: None, usable_head: None, usable_tail: None }))
    }

    pub fn init(&self, regions: MemoryRegions) {
        let mut inner = self.0.lock();
        let mut prev: Option<NonNull<Node>> = None;

        for region in regions.0.iter().filter(|region| region.typ == RegionType::Usable) {
            trace!("Initialising free list in usable region: {:?}", region);

            let node = Node { size: region.size, prev, next: None };
            let node_ptr = NonNull::new(usize::from(region.address) as *mut Node).unwrap();
            unsafe {
                node_ptr.as_ptr().write(node);
            }

            if prev.is_none() {
                assert!(inner.usable_head.is_none());
                assert!(inner.usable_tail.is_none());
                inner.usable_head = Some(node_ptr);
                inner.usable_tail = Some(node_ptr);
            } else {
                unsafe {
                    prev.as_mut().unwrap().as_mut().next = Some(node_ptr);
                }
                inner.usable_tail = Some(node_ptr);
            }

            prev = Some(node_ptr);
        }

        inner.regions = Some(regions);
    }

    pub fn walk_usable_memory(&self) {
        trace!("Tracing usable memory");
        let inner = self.0.lock();

        let mut current_node = inner.usable_head;
        while let Some(node) = current_node {
            let inner_node = unsafe { *node.as_ptr() };
            trace!("Found some usable memory at {:#x}, {} bytes of it!", node.as_ptr().addr(), inner_node.size);
            current_node = inner_node.next;
        }
        trace!("Finished tracing usable memory");
    }

    pub fn populate_memory_map(&self, memory_map: &mut seed::boot_info::MemoryMap) {
        use seed::boot_info::{MemoryMapEntry, MemoryType};

        trace!("Populating memory map from gathered regions");
        let mut inner = self.0.lock();

        for region in &inner.regions.as_ref().unwrap().0 {
            trace!("Considering region: {:?}", region);

            /*
             * First, we walk the region map.
             */
            match region.typ {
                // For usable regions, we need to check if any of it has already been allocated. We ignore these
                // regions here and instead walk the free list.
                RegionType::Usable => (),
                RegionType::Reserved(Usage::Firmware) => (),
                RegionType::Reserved(Usage::DeviceTree) => memory_map
                    .push(MemoryMapEntry::new(MemoryType::FdtReclaimable, region.address, region.size))
                    .unwrap(),
                RegionType::Reserved(Usage::Seed) => memory_map
                    .push(MemoryMapEntry::new(MemoryType::Conventional, region.address, region.size))
                    .unwrap(),
                RegionType::Reserved(Usage::KernelImage) => (),
                RegionType::Reserved(Usage::Unknown) => (),
            }
        }

        /*
         * We now walk the free list to reconstruct a map of usable memory.
         */
        let mut current_node = inner.usable_head;
        while let Some(node) = current_node {
            let inner_node = unsafe { *node.as_ptr() };
            trace!("Found some usable memory at {:#x}, {} bytes of it!", node.as_ptr().addr(), inner_node.size);
            memory_map.push(MemoryMapEntry::new(
                MemoryType::Conventional,
                PAddr::new(node.as_ptr().addr()).unwrap(),
                inner_node.size,
            ));
            current_node = inner_node.next;
        }

        /*
         * From this point, we don't want the bootloader to make any more allocations. We prevent this by removing
         * all allocatable memory.
         */
        inner.usable_head = None;
        inner.usable_tail = None;
    }
}

impl FrameAllocator<Size4KiB> for MemoryManager {
    // TODO: this doesn't currently remove empty regions, they just sit at 0 frames - I don't think this is a
    // problem tbh
    fn allocate_n(&self, n: usize) -> Range<Frame<Size4KiB>> {
        let inner = self.0.lock();
        let mut current_node = inner.usable_head;

        while let Some(node) = current_node {
            let inner_node = unsafe { &mut *node.as_ptr() };
            if inner_node.size >= (n * Size4KiB::SIZE) {
                let start_addr = node.as_ptr().addr();

                // Allocate from the end of the region so we don't need to alter the node pointers
                inner_node.size -= n * Size4KiB::SIZE;
                return Frame::starts_with(PAddr::new(start_addr + inner_node.size).unwrap())
                    ..Frame::starts_with(PAddr::new(start_addr + inner_node.size + n * Size4KiB::SIZE).unwrap());
            }

            current_node = inner_node.next;
        }

        panic!("Can't allocate {} frames :(", n);
    }

    fn free_n(&self, _start: Frame<Size4KiB>, _n: usize) {
        unimplemented!();
    }
}
