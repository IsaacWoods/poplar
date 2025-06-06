use core::ptr;

pub struct HpetRegBlock(*mut u64);

impl HpetRegBlock {
    pub const GENERAL_CAPS_AND_ID: usize = 0x00;
    pub const GENERAL_CONFIG: usize = 0x10;
    pub const GENERAL_INTERRUPT_STATUS: usize = 0x20;
    pub const MAIN_COUNTER_VALUE: usize = 0xf0;
    pub const TIMER_CONFIG_BASE: usize = 0x100;

    pub unsafe fn new(ptr: *mut u64) -> HpetRegBlock {
        HpetRegBlock(ptr)
    }

    pub fn general_caps(&self) -> GeneralCapsAndId {
        GeneralCapsAndId(unsafe { self.read_reg(Self::GENERAL_CAPS_AND_ID) })
    }

    pub fn general_config(&self) -> GeneralConfig {
        GeneralConfig(unsafe { self.read_reg(Self::GENERAL_CONFIG) })
    }

    pub fn enable_counter(&self) {
        unsafe {
            let mut config = self.general_config();
            config.set(GeneralConfig::ENABLE, true);
            self.write_reg(Self::GENERAL_CONFIG, config.bits());
        }
    }

    pub fn main_counter_value(&self) -> u64 {
        unsafe { self.read_reg(Self::MAIN_COUNTER_VALUE) }
    }

    pub unsafe fn read_reg(&self, offset: usize) -> u64 {
        unsafe { ptr::read_volatile(self.0.byte_add(offset)) }
    }

    pub unsafe fn write_reg(&self, offset: usize, value: u64) {
        unsafe {
            ptr::write_volatile(self.0.byte_add(offset), value);
        }
    }
}

mycelium_bitfield::bitfield! {
    pub struct GeneralCapsAndId<u64> {
        pub const REV_ID = 8;
        pub const MAX_TIMER = 5;
        pub const MAIN_TIMER_IS_64BIT: bool;
        const _REVERSED0: bool;
        pub const LEGACY_REPLACEMENT_CAPABLE: bool;
        pub const VENDOR_ID = 16;
        pub const COUNTER_CLK_PERIOD = 32;
    }
}

mycelium_bitfield::bitfield! {
    pub struct GeneralConfig<u64> {
        pub const ENABLE: bool;
        pub const LEGACY_REPLACEMENT: bool;
    }
}

mycelium_bitfield::bitfield! {
    pub struct TimerConfig<u64> {
        const _RESERVED0: bool;
        pub const INTERRUPT_MODE: bool;
        pub const INTERRUPT_ENABLE: bool;
        pub const PERIODIC_TIMER: bool;
        pub const SUPPORTS_PERIODIC: bool;
        pub const TIMER_IS_64BIT: bool;
        pub const SET_ACCUMULATOR: bool;
        const _RESERVED1: bool;
        pub const FORCE_32BIT: bool;
        pub const INTERRUPT_ROUTING = 5;
        pub const USE_FSB_INTERRUPT_MAPPING: bool;
        pub const SUPPORTS_FSB_INTERRUPT_MAPPING: bool;
        const _RESERVED2 = 16;
        pub const INTERRUPT_ROUTING_CAPS = 32;
    }
}
