use crate::StatusFlags;
use bit_field::BitField;
use core::sync::atomic::{self, Ordering};
use volatile::{Read, ReadWrite, Volatile};

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct VirtioVendorCap {
    pub cap_id: u8,
    pub cap_next: u8,
    pub cap_length: u8,
    pub typ: u8,
    pub bar: u8,
    pub id: u8,
    pub padding: [u8; 2],
    pub offset: u32,
    pub length: u32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[repr(u8)]
pub enum VendorCapabilityType {
    CommonCfg = 1,
    NotifyCfg = 2,
    IsrCfg = 3,
    DeviceCfg = 4,
    PciCfg = 5,
    SharedMemoryCfg = 8,
    VendorCfg = 9,
}

#[repr(C)]
pub struct VirtioPciCommonCfg {
    pub device_feature_select: Volatile<u32, ReadWrite>,
    pub device_feature: Volatile<u32, Read>,
    pub driver_feature_select: Volatile<u32, ReadWrite>,
    pub driver_feature: Volatile<u32, ReadWrite>,
    pub config_msix_vector: Volatile<u16, ReadWrite>,
    pub num_queues: Volatile<u16, ReadWrite>,
    pub device_status: Volatile<u8, ReadWrite>,
    pub config_generation: Volatile<u8, Read>,

    pub queue_select: Volatile<u16, ReadWrite>,
    pub queue_size: Volatile<u16, ReadWrite>,
    pub queue_msix_vector: Volatile<u16, ReadWrite>,
    pub queue_enable: Volatile<u16, ReadWrite>,
    pub queue_notify_off: Volatile<u16, Read>,
    pub queue_descriptor: Volatile<[u32; 2], ReadWrite>,
    pub queue_driver: Volatile<[u32; 2], ReadWrite>,
    pub queue_device: Volatile<[u32; 2], ReadWrite>,
    pub queue_notify_data: Volatile<u16, Read>,
    pub queue_reset: Volatile<u16, ReadWrite>,
}

impl VirtioPciCommonCfg {
    pub fn reset(&mut self) {
        self.device_status.write(0);
        atomic::fence(Ordering::Release);
    }

    pub fn set_status_flag(&mut self, flag: StatusFlags) {
        self.device_status.write(self.device_status.read() | flag as u8);
        atomic::fence(Ordering::Release);
    }

    pub fn is_status_flag_set(&self, flag: StatusFlags) -> bool {
        self.device_status.read() & flag as u8 == flag as u8
    }

    pub fn select_queue(&mut self, queue: u16) {
        self.queue_select.write(queue);
    }

    pub fn set_queue_size(&mut self, size: u16) {
        self.queue_size.write(size);
    }

    pub fn set_queue_msix_vector(&mut self, vector: u16) {
        self.queue_msix_vector.write(vector);
    }

    pub fn set_queue_descriptor(&mut self, physical: u64) {
        self.queue_descriptor[0].write(physical.get_bits(0..32) as u32);
        self.queue_descriptor[1].write(physical.get_bits(32..64) as u32);
    }

    pub fn set_queue_driver(&mut self, physical: u64) {
        self.queue_driver[0].write(physical.get_bits(0..32) as u32);
        self.queue_driver[1].write(physical.get_bits(32..64) as u32);
    }

    pub fn set_queue_device(&mut self, physical: u64) {
        self.queue_device[0].write(physical.get_bits(0..32) as u32);
        self.queue_device[1].write(physical.get_bits(32..64) as u32);
    }

    pub fn mark_queue_ready(&mut self) {
        // TODO: MMIO has a field called `queue_ready` - is that the same as being enabled?
        self.queue_enable.write(1);
    }
}
