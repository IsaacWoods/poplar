use core::ops::{Range, Index};
use crate::boot_services::AllocateType;
use crate::system_table;
use x86_64::memory::paging::{Frame, FrameAllocator, FRAME_SIZE};
use x86_64::memory::{PhysicalAddress, VirtualAddress};

pub struct BootFrameAllocator;

impl FrameAllocator for BootFrameAllocator {
    fn allocate_n(&self, n: usize) -> Result<Range<Frame>, !> {
        /*
         * Allocate a frame using the UEFI memory boot services. This allocator is only used by the
         * page tables code, so set the memory type to `PebblePageTables`.
         */
        let mut frame_start = PhysicalAddress::default();
        system_table()
            .boot_services
            .allocate_pages(
                AllocateType::AllocateAnyPages,
                MemoryType::PebblePageTables,
                n,
                &mut frame_start,
            )
            .unwrap();

        // Zero it for sanity's sake
        unsafe {
            system_table().boot_services.set_mem(
                u64::from(frame_start) as *mut _,
                n * (FRAME_SIZE as usize),
                0,
            );
        }

        Ok(Frame::contains(frame_start)..Frame::contains(frame_start) + n as u64)
    }

    fn free(&self, frame: Frame) {
        panic!(
            "Physical memory freed in bootloader: frame starting at {:#x}",
            frame.start_address()
        );
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
        MemoryMapIter {
            cur_index: 0,
            memory_map: memory_map,
        }
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

/// Type of memory
#[derive(Debug)]
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
     * purposes. We use a few so the OS can locate itself and things like the page tables when we
     * hand over control (this isn't how the OS *should* locate these structures [it should instead
     * use the passed `BootInformation` struct], but these values identify the used regions in the
     * memory map easily).
     */
    PebbleKernelMemory = 0x8000_0000,
    PebblePageTables = 0x8000_0001,
    PebbleBootInformation = 0x8000_0002,
}
