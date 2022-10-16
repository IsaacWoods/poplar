/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

use arrayvec::ArrayVec;
use core::{fmt, ops::Range};
use hal::memory::PhysicalAddress;
use poplar_util::ranges::RangeIntersect;
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
    pub address: PhysicalAddress,
    pub size: usize,
}

impl Region {
    pub fn new(typ: RegionType, address: PhysicalAddress, size: usize) -> Region {
        Region { typ, address, size }
    }

    pub fn usable(address: PhysicalAddress, size: usize) -> Region {
        Self::new(RegionType::Usable, address, size)
    }

    pub fn reserved(usage: Usage, address: PhysicalAddress, size: usize) -> Region {
        Self::new(RegionType::Reserved(usage), address, size)
    }

    pub fn range(&self) -> Range<PhysicalAddress> {
        self.address..(self.address + self.size)
    }
}

impl fmt::Debug for Region {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Region({:?}, {:#x}..{:#x})", self.typ, self.address, self.address + self.size)
    }
}

const MAX_REGIONS: usize = 32;

#[derive(Clone, Debug)]
pub struct MemoryManager {
    regions: ArrayVec<Region, MAX_REGIONS>,
}

impl MemoryManager {
    pub fn new() -> MemoryManager {
        MemoryManager { regions: ArrayVec::new() }
    }

    /// Add a region of memory to the manager, merging and handling intersecting regions as needed.
    pub fn add_region(&mut self, region: Region) {
        let mut added = false;

        for mut existing in &mut self.regions {
            if region.typ == existing.typ {
                /*
                 * The new region is the same type as the existing region - see if the new region is contained
                 * inside the existing one, or if we we can merge it onto the front or end.
                 * TODO: this doesn't consider the case of a new region connecting two regions so that all three
                 * can be merged - do we care?
                 */
                trace!(
                    "Comparing region {:?} against existing region {:?} since they're the same type",
                    region,
                    existing
                );
                if existing.range().encompasses(region.range()) {
                    trace!("Existing region contains new region. No action needed.");
                    added = true;
                } else if (region.address + region.size) == existing.address {
                    trace!("New region is directly before the existing region. Merging.");
                    existing.address = region.address;
                    existing.size += region.size;
                    added = true;
                } else if (existing.address + existing.size) == region.address {
                    trace!("New region is directly after the existing region. Merging");
                    existing.size += region.size;
                    added = true;
                } else {
                    trace!("Nothing we can do. Continuing to next region.");
                }
            }
        }

        /*
         * We now consider regions that aren't the same type as the region we're adding; if this new region covers
         * part of an existing region, we need to 'shrink' that region so that they don't overlap. If the new
         * region is in the middle of another, we have to split the existing region into two.
         */
        self.regions = self
            .regions
            .clone()
            .into_iter()
            .flat_map(|existing| {
                let mut new_entries = ArrayVec::<Region, 2>::new();

                if existing.typ == region.typ {
                    new_entries.push(existing);
                    return new_entries;
                }

                trace!(
                    "Comparing region {:?} against existing region {:?} as they're different types",
                    region,
                    existing
                );
                let (before, middle, after) = existing.range().split(region.range());
                if middle.is_none() {
                    // The regions don't intersect - add the existing region back
                    trace!("Regions don't intersect - add existing region back");
                    new_entries.push(existing);
                } else {
                    /*
                     * The regions do intersect, and so we need to remove the portion that intersects. Add the
                     * portions that don't intersect the new region (potentially one before and one after) back as
                     * two separate regions.
                     */
                    trace!("Regions do intersect! Splitting. before={:?}, after={:?}", before, after);
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
            trace!("Considered all regions and not yet added. Adding as a new region.");
            self.regions.push(region);
        }
    }
}
