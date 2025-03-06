use crate::{DeviceType, StatusFlags};
use bit_field::BitField;
use core::sync::atomic::{self, Ordering};
use volatile::{Read, ReadWrite, Volatile, Write};

#[repr(C)]
pub struct VirtioMmioHeader {
    pub magic: Volatile<u32, Read>,
    pub version: Volatile<u32, Read>,
    pub device_id: Volatile<u32, Read>,
    pub vendor_id: Volatile<u32, Read>,
    pub device_features: Volatile<u32, Read>,
    pub device_feature_select: Volatile<u32, Write>,
    _reserved0: [u32; 2],
    pub driver_features: Volatile<u32, Write>,
    pub driver_feature_select: Volatile<u32, Write>,
    _reserved1: [u32; 2],
    pub queue_select: Volatile<u32, Write>,
    pub queue_size_max: Volatile<u32, Read>,
    pub queue_size: Volatile<u32, ReadWrite>,
    _reserved2: [u32; 2],
    pub queue_ready: Volatile<u32, ReadWrite>,
    _reserved3: [u32; 2],
    pub queue_notify: Volatile<u32, Write>,
    _reserved4: [u32; 3],
    pub interrupt_status: Volatile<u32, Read>,
    pub interrupt_ack: Volatile<u32, Write>,
    _reserved5: [u32; 2],
    pub status: Volatile<u32, ReadWrite>,
    _reserved6: [u32; 3],
    pub queue_descriptor: Volatile<[u32; 2], ReadWrite>,
    _reserved7: [u32; 2],
    pub queue_driver: Volatile<[u32; 2], ReadWrite>,
    _reserved8: [u32; 2],
    pub queue_device: Volatile<[u32; 2], ReadWrite>,
    _reserved9: u32,
    pub shared_memory_select: Volatile<u32, Write>,
    pub shared_memory_len: Volatile<[u32; 2], Read>,
    pub shared_memory_base: Volatile<[u32; 2], Read>,
    pub queue_reset: Volatile<u32, ReadWrite>,
    _reserved10: [u32; 14],
    pub config_generation: Volatile<u32, Read>,
}

impl VirtioMmioHeader {
    pub fn reset(&mut self) {
        self.status.write(0);
    }

    pub fn set_status_flag(&mut self, flag: StatusFlags) {
        self.status.write(self.status.read() | flag as u32);
        atomic::fence(Ordering::Release);
    }

    pub fn is_magic_valid(&self) -> bool {
        self.magic.read() == u32::from_le_bytes(*b"virt")
    }

    pub fn device_type(&self) -> Result<DeviceType, ()> {
        DeviceType::try_from(self.device_id.read())
    }

    pub fn is_status_flag_set(&self, flag: StatusFlags) -> bool {
        self.status.read() & flag as u32 == flag as u32
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
        self.queue_ready.write(1);
    }
}
