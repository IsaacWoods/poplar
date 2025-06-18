use core::{cell::Cell, ops::Range, ptr};
use hal::memory::{Frame, FrameAllocator, FrameSize, PAddr, Size4KiB};
use uefi::boot::{AllocateType, MemoryType};

/// `BootFrameAllocator` is the allocator we use in the bootloader to allocate memory for the
/// kernel page tables. It pre-allocates a preset number of frames using the UEFI boot services,
/// which allows us to map things into the page tables without worrying about invalidating the
/// memory map by allocating for new entries.
///
/// We use `Cell` for interior mutability within the allocator. This is safe because the bootloader
/// is single-threaded and non-reentrant.
pub struct BootFrameAllocator {
    /// This is the first frame that cannot be allocated by this allocator
    end_frame: Frame,

    /// This points to the next frame available for allocation. When `next_frame + 1 == end_frame`,
    /// the allocator cannot allocate any more frames.
    next_frame: Cell<Frame>,
}

impl BootFrameAllocator {
    pub fn new(num_frames: usize) -> BootFrameAllocator {
        let start_frame_address =
            uefi::boot::allocate_pages(AllocateType::AnyPages, MemoryType::RESERVED, num_frames)
                .expect("Failed to allocate frames for page table allocator");

        unsafe {
            ptr::write_bytes(start_frame_address.as_ptr() as *mut u8, 0x00, num_frames * Size4KiB::SIZE);
        }

        let start_frame = Frame::contains(PAddr::new(start_frame_address.addr().get()).unwrap());
        BootFrameAllocator { end_frame: start_frame + num_frames, next_frame: Cell::new(start_frame) }
    }
}

impl FrameAllocator<Size4KiB> for BootFrameAllocator {
    fn allocate_n(&self, n: usize) -> Range<Frame> {
        if (self.next_frame.get() + n) > self.end_frame {
            panic!("Bootloader frame allocator ran out of frames!");
        }

        let frame = self.next_frame.get();
        self.next_frame.update(|frame| frame + n);

        frame..(frame + n)
    }

    fn free_n(&self, _: Frame, _: usize) {
        /*
         * NOTE: We should only free physical memory in the bootloader when we unmap the stack
         * guard page. Because of the simplicity of our allocator, we can't do anything
         * useful with the freed frame, so we just leak it.
         */
    }
}
