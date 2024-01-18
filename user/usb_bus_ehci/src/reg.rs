use bit_field::BitField;
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

bitflags! {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct Command: u32 {
        const RUN = 1 << 0;
        const RESET = 1 << 1;
        const PERIODIC_SCHEDULE_ENABLE = 1 << 4;
        const ASYNC_SCHEDULE_ENABLE = 1 << 5;
        const INTERRUPT_ON_ASYNC_ADVANCE_DOORBELL = 1 << 6;
        const LIGHT_RESET = 1 << 7;
        const ASYNC_SCHEDULE_PARK_MODE = 1 << 11;

        /*
         * Mark the Frame List Size, Async Schedule Park Mode Count, and Interrupt Threshold
         * Control fields as known bits. This makes behaviour of `bitflags`-generated methods
         * correct.
         */
        const _ = 0b111111110000001100001100;
    }
}

impl Command {
    pub fn with_interrupt_threshold(threshold: u8) -> Command {
        let mut value = 0u32;
        value.set_bits(16..24, threshold as u32);
        Command::from_bits_retain(value)
    }
}

bitflags! {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct PortStatusControl: u32 {
        const CURRENT_CONNECT_STATUS = 1 << 0;
        const CONNECT_STATUS_CHANGE = 1 << 1;
        const PORT_ENABLED = 1 << 2;
        const PORT_ENABLED_CHANGE = 1 << 3;
        const OVER_CURRENT_ACTIVE = 1 << 4;
        const OVER_CURRENT_CHANGE = 1 << 5;
        const FORCE_PORT_RESUME = 1 << 6;
        const SUSPEND = 1 << 7;
        const PORT_RESET = 1 << 8;
        const PORT_POWER = 1 << 12;
        const PORT_OWNER = 1 << 13;

        /*
         * Mark the Line Status field (bits 10..12), Port Indicator Control (bits 14..16), and Port
         * Test Control (bits 16..20) as known bits.
         */
        const _ = 0b11111100110000000000;
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineStatus {
    Se0,
    JState,
    KState,
    Undefined,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PortIndicatorControl {
    Off,
    Amber,
    Green,
    Undefined,
}

impl PortStatusControl {
    pub fn line_status(self) -> LineStatus {
        match self.bits().get_bits(10..12) {
            0b00 => LineStatus::Se0,
            0b01 => LineStatus::KState,
            0b10 => LineStatus::JState,
            0b11 => LineStatus::Undefined,
            _ => unreachable!(),
        }
    }

    pub fn port_indicator_control(self) -> PortIndicatorControl {
        match self.bits().get_bits(14..16) {
            0b00 => PortIndicatorControl::Off,
            0b01 => PortIndicatorControl::Amber,
            0b10 => PortIndicatorControl::Green,
            0b11 => PortIndicatorControl::Undefined,
            _ => unreachable!(),
        }
    }

    pub fn port_test_control(self) -> u8 {
        self.bits().get_bits(16..20) as u8
    }
}
