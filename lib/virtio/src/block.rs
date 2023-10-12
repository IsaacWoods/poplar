use crate::VirtioMmioHeader;
use volatile::{Read, Volatile};

#[repr(C)]
pub struct BlockDeviceConfig {
    // TODO: how to abstract over both MMIO and PCI Virtio devices?
    pub header: VirtioMmioHeader,
    pub capacity: Volatile<[u32; 2], Read>,
    pub size_max: Volatile<u32, Read>,
    pub seg_max: Volatile<u32, Read>,
    pub geometry: Volatile<Geometry, Read>,
    pub block_size: Volatile<u32, Read>,
    pub topology: Volatile<Topology, Read>,
    pub writeback: Volatile<u8, Read>,
    _reserved0: u8,
    pub num_queues: Volatile<u16, Read>,
    pub max_discard_sectors: Volatile<u32, Read>,
    pub max_discard_seg: Volatile<u32, Read>,
    pub discard_sector_alignment: Volatile<u32, Read>,
    pub max_write_zeroes_sectors: Volatile<u32, Read>,
    pub max_write_zeroes_seg: Volatile<u32, Read>,
    pub write_zeroes_may_unmap: Volatile<u8, Read>,
    _reserved1: [u8; 3],
    pub max_secure_erase_sectors: Volatile<u32, Read>,
    pub max_secure_erase_seg: Volatile<u32, Read>,
    pub secure_erase_sector_alignment: Volatile<u32, Read>,
}

impl BlockDeviceConfig {
    pub fn capacity(&self) -> u64 {
        let [lo, hi] = self.capacity.read();
        (u64::from(hi) << 32) + u64::from(lo)
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Geometry {
    pub cylinders: u16,
    pub heads: u8,
    pub sectors: u8,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Topology {
    /// The number of logical blocks per physical block (log2)
    pub physical_block_log2: u8,
    /// The offset of the first aligned logical block
    pub alignment_offset: u8,
    /// The minimum I/O size (in blocks)
    pub min_io_size: u16,
    /// The optimal (and suggested maximum) I/O size (in blocks)
    pub optimal_io_size: u32,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Request {
    pub typ: RequestType,
    _reserved0: u32,
    pub sector: u64,
    // XXX: various request types have a variable-size data field here. Not sure how to model that tbh.
    pub status: RequestStatus,
}

impl Request {
    pub fn read(sector: u64) -> Request {
        Request { typ: RequestType::Read, _reserved0: 0, sector, status: RequestStatus::Ok }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum RequestType {
    Read = 0,
    Write = 1,
    Flush = 4,
    GetId = 8,
    GetLifetime = 10,
    Discard = 11,
    WriteZeroes = 13,
    SecureErase = 14,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum RequestStatus {
    Ok = 0,
    Error = 1,
    Unsupported = 2,
}
