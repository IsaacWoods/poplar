#![feature(never_type)]

mod caps;

use crate::caps::Capabilities;
use bit_field::BitField;
use log::{info, trace};
use platform_bus::{BusDriverMessage, DeviceDriverMessage, DeviceDriverRequest, Filter, Property};
use std::{
    mem,
    poplar::{
        caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING, CAP_SERVICE_USER},
        channel::Channel,
        early_logger::EarlyLogger,
        memory_object::MemoryObject,
        syscall::{self, MemoryObjectFlags},
    },
};

pub struct Controller {
    register_base: usize,
    caps: Capabilities,
    free_addresses: Vec<u8>,
}

impl Controller {
    pub fn new(register_base: usize) -> Controller {
        let caps = unsafe { Capabilities::read_from_registers(register_base) };
        info!("Capabilites: {:#?}", caps);

        Controller { register_base, caps, free_addresses: (1..128).collect() }
    }

    pub fn initialize(&mut self) {
        // TODO: something else may have used the controller. We should probs halt and reset it.
        // However, this requires access to the PCI config space so will need functionality probs
        // involving the PCI bus driver (or to hand-over the space?)?

        /*
         * We only support controllers that don't support 64-bit addressing at the moment. This
         * means we don't need to set `CTRLDSSEGMENT`.
         */
        assert!(!self.caps.can_address_64bit);

        unsafe {
            // Enable interrupts we're interested in
            self.write_operational_register(
                OpRegister::InterruptEnable,
                (InterruptEnable::INTERRUPT
                    | InterruptEnable::ERROR
                    | InterruptEnable::PORT_CHANGE
                    | InterruptEnable::HOST_ERROR)
                    .bits(),
            );

            // Turn the controller on
            self.write_operational_register(
                OpRegister::Command,
                (Command::RUN | Command::with_interrupt_threshold(0x08)).bits(),
            );

            // Route all ports to the EHCI controller
            self.write_operational_register(OpRegister::ConfigFlag, 1);
        }
    }

    pub fn check_ports(&mut self) {
        // Check each port
        assert!(!self.caps.port_power_control, "We don't support port power control");
        for port in 0..self.caps.num_ports {
            let port_reg = unsafe { self.read_port_register(port) };
            info!("Port status/cmd for port {}: {:#x} ({:#0b})", port, port_reg, port_reg);

            if port_reg.contains(PortStatusControl::CONNECT_STATUS_CHANGE) {
                // Clear the changed status by writing a `1` to it
                unsafe {
                    self.write_port_register(port, PortStatusControl::CONNECT_STATUS_CHANGE);
                }

                if port_reg.contains(PortStatusControl::CURRENT_CONNECT_STATUS) {
                    // Read the initial state of the D+/D- pins. This allows us to detect Low-Speed
                    // devices before resetting the port.
                    if port_reg.line_status() == LineStatus::KState {
                        /*
                         * The line being in K-state means the connected device is Low-Speed. It
                         * must be handed off to a companion controller.
                         */
                        trace!("Device on port {} is low-speed. Handing off to companion controller.", port);
                        unsafe {
                            self.write_port_register(port, PortStatusControl::PORT_OWNER);
                        }
                    } else {
                        /*
                         * All line states except K-state mean the connected device is not
                         * Low-Speed. We can start the port reset and enable sequence.
                         */
                        trace!("Connected device on port {}", port);
                        self.handle_device_connect(port);
                    }
                } else {
                    trace!("Device on port {} disconnected", port);
                }
            }
        }
    }

    pub fn handle_device_connect(&mut self, port: u8) {
        self.reset_port(port);

        unsafe {
            if self.read_port_register(port).contains(PortStatusControl::PORT_ENABLED) {
                // The device is High-Speed
                trace!("Device on port {} is high-speed.", port);
            } else {
                /*
                 * The device is not High-Speed. Hand it off to a companion controller to deal
                 * with.
                 */
                trace!("Device on port {} is full-speed. Handing off to companion controller.", port);
                self.write_port_register(port, PortStatusControl::PORT_OWNER);
            }
        }
    }

    pub fn reset_port(&mut self, port: u8) {
        unsafe {
            /*
             * Reset the port by toggling the PortReset bit and then waiting for it to clear.
             */
            self.write_port_register(port, PortStatusControl::PORT_RESET);
            // TODO: apparently we're meant to time a duration here???? QEMU doesn't complain about
            // no delay but I bet real ones do
            self.write_port_register(port, PortStatusControl::empty());
            while self.read_port_register(port).contains(PortStatusControl::PORT_RESET) {}
        }
    }

    pub unsafe fn write_operational_register(&mut self, reg: OpRegister, value: u32) {
        let address = self.register_base + self.caps.cap_length as usize + (reg as u32 as usize);
        unsafe {
            std::ptr::write_volatile(address as *mut u32, value);
        }
    }

    pub unsafe fn read_port_register(&self, port: u8) -> PortStatusControl {
        let address =
            self.register_base + self.caps.cap_length as usize + 0x44 + mem::size_of::<u32>() * port as usize;
        PortStatusControl::from_bits_truncate(unsafe { std::ptr::read_volatile(address as *const u32) })
    }

    pub unsafe fn write_port_register(&self, port: u8, value: PortStatusControl) {
        let address =
            self.register_base + self.caps.cap_length as usize + 0x44 + mem::size_of::<u32>() * port as usize;
        unsafe {
            std::ptr::write_volatile(address as *mut u32, value.bits());
        }
    }
}

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

bitflags::bitflags! {
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

bitflags::bitflags! {
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

bitflags::bitflags! {
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

fn main() {
    log::set_logger(&EarlyLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("EHCI USB Bus Driver is running!");

    // This allows us to talk to the PlatformBus as a bus driver (to register USB devices).
    let platform_bus_bus_channel: Channel<BusDriverMessage, !> =
        Channel::from_handle(syscall::subscribe_to_service("platform_bus.bus_driver").unwrap());
    // This allows us to talk to the PlatformBus as a device driver (to find controllers we can manage).
    let platform_bus_device_channel: Channel<DeviceDriverMessage, DeviceDriverRequest> =
        Channel::from_handle(syscall::subscribe_to_service("platform_bus.device_driver").unwrap());

    // Tell PlatformBus that we're interested in XHCI controllers.
    platform_bus_device_channel
        .send(&DeviceDriverMessage::RegisterInterest(vec![
            Filter::Matches(String::from("pci.class"), Property::Integer(0x0c)),
            Filter::Matches(String::from("pci.sub_class"), Property::Integer(0x03)),
            Filter::Matches(String::from("pci.interface"), Property::Integer(0x20)),
        ]))
        .unwrap();

    // TODO: we currently only support one controller, and just stop listening after we find the first one
    // TODO: probably don't bother changing this until we have a futures-based message interface
    let mut controller_device = loop {
        match platform_bus_device_channel.try_receive().unwrap() {
            Some(DeviceDriverRequest::HandoffDevice(device_name, device)) => {
                info!("Started driving a EHCI controller: {}", device_name);
                break device;
            }
            None => syscall::yield_to_kernel(),
        }
    };

    let register_space_size =
        controller_device.properties.get("pci.bar0.size").unwrap().as_integer().unwrap() as usize;

    // TODO: let the kernel choose the address when it can - we don't care
    // TODO: this trusts the data from the platform_bus. Maybe we shouldn't do that? One
    // idea would be a syscall for querying info about the object?
    let register_space = MemoryObject {
        handle: controller_device.properties.get("pci.bar0.handle").as_ref().unwrap().as_memory_object().unwrap(),
        size: register_space_size,
        flags: MemoryObjectFlags::WRITABLE,
        phys_address: None,
    };
    const REGISTER_SPACE_ADDRESS: usize = 0x00000005_00000000;
    unsafe {
        register_space.map_at(REGISTER_SPACE_ADDRESS).unwrap();
    }

    let mut controller = Controller::new(REGISTER_SPACE_ADDRESS);
    controller.initialize();
    controller.check_ports();

    loop {
        std::poplar::syscall::yield_to_kernel();
    }
}

#[used]
#[link_section = ".caps"]
pub static mut CAPS: CapabilitiesRepr<4> =
    CapabilitiesRepr::new([CAP_EARLY_LOGGING, CAP_SERVICE_USER, CAP_PADDING, CAP_PADDING]);
