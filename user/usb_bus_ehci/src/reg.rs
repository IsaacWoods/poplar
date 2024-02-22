use bitflags::bitflags;

#[repr(u32)]
pub enum OpRegister {
    Command = 0x00,
    Status = 0x04,
    InterruptEnable = 0x08,
    FrameIndex = 0x0c,
    LongSegmentSelector = 0x10,
    FrameListBaseAddress = 0x14,
    NextAsyncListAddress = 0x18,
    ConfigFlag = 0x40,
    PortBase = 0x44,
}

bitflags! {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct InterruptEnable: u32 {
        const INTERRUPT = 1 << 0;
        const ERROR = 1 << 1;
        const PORT_CHANGE = 1 << 2;
        const FRAME_LIST_ROLLOVER = 1 << 3;
        const HOST_ERROR = 1 << 4;
        const ON_ASYNC_ADVANCE = 1 << 5;
    }
}

mycelium_bitfield::bitfield! {
    pub struct Command<u32> {
        pub const RUN: bool;
        pub const RESET: bool;
        pub const FRAME_LIST_SIZE = 2;
        pub const PERIODIC_SCHEDULE_ENABLE: bool;
        pub const ASYNC_SCHEDULE_ENABLE: bool;
        pub const INTERRUPT_ON_ASYNC_ADVANCE_DOORBELL: bool;
        pub const LIGHT_RESET: bool;
        pub const ASYNC_SCHEDULE_PARK_MODE_COUNT = 2;
        pub const _RESERVED0 = 1;
        pub const ASYNC_SCHEDULE_PARK_MODE: bool;
        pub const _RESERVED1 = 4;
        pub const INTERRUPT_THRESHOLD = 8;
    }
}

mycelium_bitfield::bitfield! {
    pub struct Status<u32> {
        pub const INTERRUPT: bool;
        pub const ERR_INTERRUPT: bool;
        pub const PORT_CHANGE_DETECT: bool;
        pub const FRAME_LIST_ROLLOVER: bool;
        pub const HOST_SYSTEM_ERR: bool;
        pub const INTERRUPT_ON_ASYNC_ADVANCE: bool;
        const _RESERVED0 = 6;
        pub const CONTROLLER_HALTED: bool;
        pub const RECLAMATION: bool;
        pub const PERIODIC_SCHEDULE_STATUS: bool;
        pub const ASYNC_SCHEDULE_STATUS: bool;
    }
}

mycelium_bitfield::bitfield! {
    pub struct PortStatusControl<u32> {
        pub const CURRENT_CONNECT_STATUS: bool;
        pub const CONNECT_STATUS_CHANGE: bool;
        pub const PORT_ENABLED: bool;
        pub const PORT_ENABLED_CHANGE: bool;
        pub const OVER_CURRENT_ACTIVE: bool;
        pub const OVER_CURRENT_CHANGE: bool;
        pub const FORCE_PORT_RESUME: bool;
        pub const SUSPEND: bool;
        pub const PORT_RESET: bool;
        pub const _RESERVED0 = 1;
        pub const LINE_STATUS: LineStatus;
        pub const PORT_POWER: bool;
        pub const PORT_OWNER: bool;
        pub const PORT_INDICATOR_CONTROL: PortIndicatorControl;
        pub const PORT_TEST_CONTROL = 4;
        pub const WAKE_ON_CONNECT_ENABLE: bool;
        pub const WAKE_ON_DISCONNECT_ENABLE: bool;
        pub const WAKE_ON_OVERCURRENT: bool;
    }
}

mycelium_bitfield::enum_from_bits! {
    #[derive(PartialEq, Debug)]
    pub enum LineStatus<u8> {
        Se0 = 0b00,
        JState = 0b10,
        KState = 0b01,
        Undefined = 0b11,
    }
}

mycelium_bitfield::enum_from_bits! {
    #[derive(Debug)]
    pub enum PortIndicatorControl<u8> {
        Off = 0b00,
        Amber = 0b01,
        Green = 0b10,
        Undefined = 0b11,
    }
}
