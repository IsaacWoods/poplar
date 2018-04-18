/*
 * Copyright (C) 2017, Pebble Developers.
 * See LICENCE.md
 */

use super::Frame;
use super::paging::PhysicalAddress;
use multiboot2::{MemoryAreaIter,MemoryArea};

pub struct FrameAllocator
{
    next_free_frame : Frame,
    current_area    : Option<&'static MemoryArea>,
    areas           : MemoryAreaIter,
    multiboot_start : Frame,
    multiboot_end   : Frame,
    kernel_start    : Frame,
    kernel_end      : Frame,
}

impl FrameAllocator
{
    pub fn new(multiboot_start  : PhysicalAddress,
               multiboot_end    : PhysicalAddress,
               kernel_start     : PhysicalAddress,
               kernel_end       : PhysicalAddress,
               memory_areas     : MemoryAreaIter) -> FrameAllocator
    {
        let mut allocator = FrameAllocator
                            {
                                next_free_frame : Frame::containing_frame(0.into()),
                                current_area    : None,
                                areas           : memory_areas,
                                multiboot_start : Frame::containing_frame(multiboot_start),
                                multiboot_end   : Frame::containing_frame(multiboot_end),
                                kernel_start    : Frame::containing_frame(kernel_start),
                                kernel_end      : Frame::containing_frame(kernel_end),
                            };

        allocator.switch_to_next_area();
        allocator
    }

    fn switch_to_next_area(&mut self)
    {
        self.current_area = self.areas.clone().filter(
            |area| {
                let address = area.start_address() + area.size() + 1;
                Frame::containing_frame((address as usize).into()) >= self.next_free_frame
            }).min_by_key(|area| area.start_address());

        if let Some(area) = self.current_area
        {
            let start_frame = Frame::containing_frame((area.start_address() as usize).into());
            if self.next_free_frame < start_frame
            {
                self.next_free_frame = start_frame;
            }
        }
    }

    pub fn allocate_frame(&mut self) -> Option<Frame>
    {
        if let Some(area) = self.current_area
        {
            // Keep the next free frame to return it if it's free
            let frame = self.next_free_frame;

            // The last frame of the current area
            let current_area_last_frame = Frame::containing_frame(((area.start_address() + area.size() - 1) as usize).into());

            if frame > current_area_last_frame
            {
                // We've run out of frames in this area, switch to the next one
                self.switch_to_next_area();
            }
            else if frame >= self.kernel_start && frame <= self.kernel_end
            {
                self.next_free_frame = Frame { number : self.kernel_end.number + 1 };
            }
            else if frame >= self.multiboot_start && frame <= self.multiboot_end
            {
                self.next_free_frame = Frame { number : self.multiboot_end.number + 1 };
            }
            else
            {
                self.next_free_frame.number += 1;
                return Some(frame);
            }

            self.allocate_frame()
        }
        else
        {
            // There are no more free frames
            None
        }
    }

    /// Allocates a contiguous block of frames, if possible. Returns `None` if a contiguous
    /// allocation of that size is not possible. On success, returns a tuple where the first
    /// element is the first frame, and the second element is the last frame.
    pub fn allocate_frame_block(&mut self, block_size : usize) -> Option<(Frame, Frame)>
    {
        if let Some(area) = self.current_area
        {
            /*
             * We're looking for an area with enough free contiguous frames to satisfy the
             * allocation. If the current area doesn't, we switch to the next one.
             *
             * XXX TODO FIXME: This is terrible way of doing it, especially if we try and make
             * large allocations, because it will skip over areas permanently that could fit
             * smaller allocations. This must be fixed when we iterate the physical memory manager.
             */
            let frame = self.next_free_frame;
            let current_area_last_frame = Frame::containing_frame(((area.start_address() + area.size() - 1) as usize).into());
            let block_last_frame = self.next_free_frame + (block_size - 1);

            // XXX: We ignore the kernel and multiboot structure reservations for now, and just
            // hope we don't hit them
            if current_area_last_frame < block_last_frame
            {
                // Allocation doesn't fit, switch to the next area
                self.switch_to_next_area();
                self.allocate_frame_block(block_size)
            }
            else
            {
                self.next_free_frame = block_last_frame + 1;
                Some((frame, block_last_frame))
            }
        }
        else
        {
            // There are no more free frames
            None
        }
    }

    pub fn deallocate_frame(&mut self, _frame : Frame)
    {
        /*
         * NOTE: A better frame allocator would track freed frames to reallocate later, but we
         * don't bother.
         */
    }
}
