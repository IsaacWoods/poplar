use crate::uefi::system_table;
use core::{
    cell::Cell,
    ops::{Index, Range},
};
use x86_64::memory::{Frame, FrameAllocator, FrameSize, PhysicalAddress, Size4KiB, VirtualAddress};

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
            match system_table().boot_services.allocate_frames(MemoryType::PebblePageTables, num_frames) {
                Ok(address) => address,
                Err(err) => panic!("Failed to allocate memory for page frame allocator: {:?}", err),
            };

        // Zero all the memory so the page tables start with everything unmapped
        unsafe {
            system_table().boot_services.set_mem(
                usize::from(start_frame_address) as *mut _,
                num_frames * Size4KiB::SIZE,
                0,
            );
        }

        let start_frame = Frame::contains(start_frame_address);
        BootFrameAllocator { end_frame: start_frame + num_frames, next_frame: Cell::new(start_frame) }
    }
}

impl FrameAllocator for BootFrameAllocator {
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

/// Describes a region of memory
#[derive(Debug)]
#[repr(C)]
pub struct MemoryDescriptor {
    pub memory_type: MemoryType,
    pub physical_start: PhysicalAddress,
    pub virtual_start: VirtualAddress,
    pub number_of_pages: u64,
    pub attribute: u64, // TODO: bitflags
}

/// Describes the system's current memory configuration
#[derive(Debug)]
pub struct MemoryMap {
    pub buffer: *mut MemoryDescriptor,
    pub descriptor_size: usize,
    pub descriptor_version: u32,
    pub key: usize,
    pub size: usize,
}

impl MemoryMap {
    pub fn iter(&self) -> impl Iterator<Item = &MemoryDescriptor> {
        MemoryMapIter::new(self)
    }

    #[inline]
    pub fn num_entries(&self) -> usize {
        self.size / self.descriptor_size
    }
}

impl Index<usize> for MemoryMap {
    type Output = MemoryDescriptor;

    fn index(&self, index: usize) -> &MemoryDescriptor {
        let index = index * self.descriptor_size;
        if index + self.descriptor_size > self.size {
            panic!("MemoryMap index out of bounds");
        }

        unsafe {
            let addr = (self.buffer as usize) + index;
            (addr as *mut MemoryDescriptor).as_ref().unwrap()
        }
    }
}

struct MemoryMapIter<'a> {
    cur_index: usize,
    memory_map: &'a MemoryMap,
}

impl<'a> MemoryMapIter<'a> {
    fn new(memory_map: &MemoryMap) -> MemoryMapIter {
        MemoryMapIter { cur_index: 0, memory_map }
    }
}

impl<'a> Iterator for MemoryMapIter<'a> {
    type Item = &'a MemoryDescriptor;

    fn next(&mut self) -> Option<&'a MemoryDescriptor> {
        if self.cur_index < self.memory_map.num_entries() {
            let desc = &self.memory_map[self.cur_index];
            self.cur_index += 1;
            Some(desc)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum MemoryType {
    ReservedMemoryType,
    LoaderCode,
    LoaderData,
    BootServicesCode,
    BootServicesData,
    RuntimeServicesCode,
    RuntimeServicesData,
    ConventionalMemory,
    UnusableMemory,
    ACPIReclaimMemory,
    ACPIMemoryNVS,
    MemoryMappedIO,
    MemoryMappedIOPortSpace,
    PalCode,
    PersistentMemory,
    MaxMemoryType,

    /*
     * Values between 0x8000_0000 and 0xffff_ffff are free to use by OS loaders for their own
     * purposes.
     */
    PebbleKernelMemory = 0x8000_0000,
    PebblePageTables = 0x8000_0001,
    PebbleBootInformation = 0x8000_0002,
    PebbleKernelHeap = 0x8000_0003,
    PebbleImageMemory = 0x8000_0004,
}
