#![no_std]

pub mod block;

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
    pub queue_num_max: Volatile<u32, Read>,
    pub queue_num: Volatile<u32, Read>,
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
    pub queue_descriptor: Volatile<[u32; 2], Write>,
    _reserved7: [u32; 2],
    pub queue_driver: Volatile<[u32; 2], Write>,
    _reserved8: [u32; 2],
    pub queue_device: Volatile<[u32; 2], Write>,
    _reserved9: u32,
    pub shared_memory_select: Volatile<u32, Write>,
    pub shared_memory_len: Volatile<[u32; 2], Read>,
    pub shared_memory_base: Volatile<[u32; 2], Read>,
    pub queue_reset: Volatile<u32, ReadWrite>,
    _reserved10: [u32; 14],
    pub config_generation: Volatile<u32, Read>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum DeviceType {
    Invalid = 0,
    NetworkCard = 1,
    BlockDevice = 2,
    Console = 3,
    EntropySource = 4,
    TraditionalMemoryBalloon = 5,
    IoMemory = 6,
    Rpmsg = 7,
    ScsiHost = 8,
    Transport9P = 9,
    Mac80211Wlan = 10,
    RProcSerial = 11,
    VirtioCaif = 12,
    MemoryBalloon = 13,
    GpuDevice = 16,
    TimerDevice = 17,
    InputDevice = 18,
    SocketDevice = 19,
    CryptoDevice = 20,
    SignalDistributionModule = 21,
    PStoreDevice = 22,
    IommuDevice = 23,
    MemoryDevice = 24,
    AudioDevice = 25,
    FileSystemDevice = 26,
    PmemDevice = 27,
    RpmbDevice = 28,
    Mac80211HwsimWirelessSimulationDevice = 29,
    VideoEncoderDevice = 30,
    VideoDecoderDevice = 31,
    ScmiDevice = 32,
    NitroSecureModule = 33,
    I2CAdaptor = 34,
    Watchdog = 35,
    CanDevice = 36,
    ParameterServer = 38,
    AudioPolicyDevice = 39,
    BluetoothDevice = 40,
    GpioDevice = 41,
    RdmaDevice = 42,
}

impl core::convert::TryFrom<u32> for DeviceType {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(DeviceType::Invalid),
            1 => Ok(DeviceType::NetworkCard),
            2 => Ok(DeviceType::BlockDevice),
            3 => Ok(DeviceType::Console),
            4 => Ok(DeviceType::EntropySource),
            5 => Ok(DeviceType::TraditionalMemoryBalloon),
            6 => Ok(DeviceType::IoMemory),
            7 => Ok(DeviceType::Rpmsg),
            8 => Ok(DeviceType::ScsiHost),
            9 => Ok(DeviceType::Transport9P),
            10 => Ok(DeviceType::Mac80211Wlan),
            11 => Ok(DeviceType::RProcSerial),
            12 => Ok(DeviceType::VirtioCaif),
            13 => Ok(DeviceType::MemoryBalloon),
            16 => Ok(DeviceType::GpuDevice),
            17 => Ok(DeviceType::TimerDevice),
            18 => Ok(DeviceType::InputDevice),
            19 => Ok(DeviceType::SocketDevice),
            20 => Ok(DeviceType::CryptoDevice),
            21 => Ok(DeviceType::SignalDistributionModule),
            22 => Ok(DeviceType::PStoreDevice),
            23 => Ok(DeviceType::IommuDevice),
            24 => Ok(DeviceType::MemoryDevice),
            25 => Ok(DeviceType::AudioDevice),
            26 => Ok(DeviceType::FileSystemDevice),
            27 => Ok(DeviceType::PmemDevice),
            28 => Ok(DeviceType::RpmbDevice),
            29 => Ok(DeviceType::Mac80211HwsimWirelessSimulationDevice),
            30 => Ok(DeviceType::VideoEncoderDevice),
            31 => Ok(DeviceType::VideoDecoderDevice),
            32 => Ok(DeviceType::ScmiDevice),
            33 => Ok(DeviceType::NitroSecureModule),
            34 => Ok(DeviceType::I2CAdaptor),
            35 => Ok(DeviceType::Watchdog),
            36 => Ok(DeviceType::CanDevice),
            38 => Ok(DeviceType::ParameterServer),
            39 => Ok(DeviceType::AudioPolicyDevice),
            40 => Ok(DeviceType::BluetoothDevice),
            41 => Ok(DeviceType::GpioDevice),
            42 => Ok(DeviceType::RdmaDevice),
            _ => Err(()),
        }
    }
}

#[repr(C)]
pub struct BlockDeviceConfig {
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
