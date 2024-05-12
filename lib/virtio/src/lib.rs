#![no_std]
#![feature(slice_ptr_get, layout_for_ptr, ptr_metadata)]

extern crate alloc;

pub mod block;
pub mod gpu;
pub mod mmio;
pub mod pci;
pub mod virtqueue;

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum StatusFlags {
    Acknowledge = 1,
    Driver = 2,
    DriverOk = 4,
    FeaturesOk = 8,
    NeedsReset = 64,
    Failed = 128,
}
