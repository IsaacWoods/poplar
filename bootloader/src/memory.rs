use core::ops;

pub type PhysicalAddress = *mut u8;
pub type VirtualAddress = u64;

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

impl ops::Index<usize> for MemoryMap {
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
}
