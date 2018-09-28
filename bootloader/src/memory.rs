#[repr(C)]
pub enum MemoryType {
    ReservedMemoryType,
    LoaderCode,
    LoaderData,
    BootServicesCode,
    BootServicesData,
    RuntimeServicesCode,
    RuntimeServicesData,
    Conventional,
    Unusable,
    AcpiReclaimable,
    AcpiNvs,
    MemoryMappedIo,
    MemoryMappedIoPortSpace,
    PalCode,
    Persistent,

    MaxMemoryType,
}

#[repr(C)]
pub struct MemoryDescriptor {
    pub descriptor_type: u32,
    pub physical_start: u64,
    pub virtual_start: u64,
    pub num_pages: u64,
    pub attribute: u64,
}
