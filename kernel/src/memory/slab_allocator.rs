use alloc::vec::Vec;
use hal::memory::VAddr;
use mulch::{bitmap::BitmapSlice, math::ceiling_integer_divide};

pub struct SlabAllocator {
    pub bottom: VAddr,
    pub top: VAddr,
    slab_size: usize,
    bitmap: Vec<u8>,
}

impl SlabAllocator {
    pub fn new(bottom: VAddr, top: VAddr, slab_size: usize) -> SlabAllocator {
        let num_bytes_needed = ceiling_integer_divide(usize::from(top) - usize::from(bottom), slab_size) / 8;
        SlabAllocator { bottom, top, slab_size, bitmap: vec![0; num_bytes_needed] }
    }

    /// Try to allocate a slab out of the allocator. Returns `None` if no slabs are available.
    pub fn alloc(&mut self) -> Option<VAddr> {
        let index = self.bitmap.alloc(1)?;
        Some(self.bottom + index * self.slab_size)
    }

    pub fn free(&mut self, start: VAddr) {
        assert_eq!((usize::from(start) - usize::from(self.bottom)) % self.slab_size, 0);
        let index = (usize::from(start) - usize::from(self.bottom)) / self.slab_size;
        self.bitmap.free(index, 1);
    }
}
