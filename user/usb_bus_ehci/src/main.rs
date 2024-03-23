//! `usb_bus_ehci` is a driver compatible with EHCI USB host controllers.
//!
//! ### Development
//!    - On QEMU, enabling tracing of `usb_ehci_*` events is helpful for debugging.

#![feature(never_type)]

mod caps;
mod queue;
mod reg;

use crate::{
    caps::Capabilities,
    queue::{Queue, QueueHead},
    reg::{Command, InterruptEnable, LineStatus, OpRegister, PortStatusControl, Status},
};
use log::{info, trace};
use platform_bus::{
    BusDriverMessage,
    DeviceDriverMessage,
    DeviceDriverRequest,
    DeviceInfo,
    Filter,
    HandoffInfo,
    HandoffProperty,
    Property,
};
use queue::HorizontalLinkPtr;
use std::{
    collections::BTreeMap,
    mem,
    poplar::{
        caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING, CAP_SERVICE_USER},
        channel::Channel,
        ddk::dma::{DmaObject, DmaPool},
        early_logger::EarlyLogger,
        memory_object::MemoryObject,
        syscall::MemoryObjectFlags,
        Handle,
    },
    sync::Arc,
};
use usb::{
    descriptor::{ConfigurationDescriptor, DescriptorType, DeviceDescriptor},
    setup::{Direction, Recipient, Request, RequestType, RequestTypeType, SetupPacket},
    DeviceControlMessage,
};

pub struct Controller {
    register_base: usize,
    caps: Capabilities,
    free_addresses: Vec<u8>,
    schedule_pool: DmaPool,
    active_devices: BTreeMap<u8, ActiveDevice>,
    platform_bus_bus_channel: Arc<Channel<BusDriverMessage, !>>,
    interrupt_event: Handle,
}

pub struct ActiveDevice {
    control_queue: Queue,
}

impl Controller {
    pub fn new(
        register_base: usize,
        platform_bus_bus_channel: Arc<Channel<BusDriverMessage, !>>,
        interrupt_event: Handle,
    ) -> Controller {
        let caps = Capabilities::read_from_registers(register_base);
        info!("Capabilites: {:#?}", caps);

        // TODO: once we have kernel virtual address space management, just let it find an address
        // for us
        const SCHEDULE_POOL_ADDRESS: usize = 0x00000005_10000000;
        let schedule_pool = DmaPool::new(unsafe {
            MemoryObject::create_physical(0x1000, MemoryObjectFlags::WRITABLE)
                .unwrap()
                .map_at(SCHEDULE_POOL_ADDRESS)
                .unwrap()
        });

        Controller {
            register_base,
            caps,
            free_addresses: (1..128).collect(),
            schedule_pool,
            active_devices: BTreeMap::new(),
            platform_bus_bus_channel,
            interrupt_event,
        }
    }

    pub fn initialize(&mut self) {
        /*
         * We only support controllers that don't support 64-bit addressing at the moment. This
         * means we don't need to set `CTRLDSSEGMENT`.
         */
        assert!(!self.caps.can_address_64bit);

        // If the controller has already been used by the firmware, halt it before trying to reset
        if !self.read_status().get(Status::CONTROLLER_HALTED) {
            info!("EHCI controller has already been started. Halting it.");
            let command = self.read_command();
            unsafe {
                self.write_command(
                    Command::new()
                        .with(Command::RUN, false)
                        .with(Command::INTERRUPT_THRESHOLD, command.get(Command::INTERRUPT_THRESHOLD)),
                );
            }
            while !self.read_status().get(Status::CONTROLLER_HALTED) {}
        }

        // Reset the controller
        unsafe {
            self.write_command(Command::new().with(Command::RESET, true).with(Command::INTERRUPT_THRESHOLD, 0x08));
            while self.read_command().get(Command::RESET) {}
        }
        info!("EHCI controller reset");

        unsafe {
            // Enable interrupts we're interested in
            self.write_operational_register(
                OpRegister::InterruptEnable,
                (InterruptEnable::INTERRUPT
                    | InterruptEnable::ERROR
                    // | InterruptEnable::PORT_CHANGE       // TODO: we actually need to handle +
                                // acknowledge this otherwise the controller seems to get confused and won't
                                // signal transaction interrupts correctly
                    | InterruptEnable::HOST_ERROR)
                    .bits(),
            );

            // Turn the controller on
            self.write_command(Command::new().with(Command::RUN, true).with(Command::INTERRUPT_THRESHOLD, 0x08));

            // Route all ports to the EHCI controller
            self.write_operational_register(OpRegister::ConfigFlag, 1);
        }
    }

    pub fn check_ports(&mut self) {
        assert!(!self.caps.port_power_control, "We don't support port power control");

        for port in 0..self.caps.num_ports {
            let port_reg = unsafe { self.read_port_register(port) };

            if port_reg.get(PortStatusControl::PORT_ENABLED_CHANGE) {
                // TODO: handle this better
                info!("Port error on port {}", port);
            }

            if port_reg.get(PortStatusControl::CONNECT_STATUS_CHANGE) {
                // Clear the changed status by writing a `1` to it
                unsafe {
                    self.write_port_register(
                        port,
                        PortStatusControl::new().with(PortStatusControl::CONNECT_STATUS_CHANGE, true),
                    );
                }

                if port_reg.get(PortStatusControl::CURRENT_CONNECT_STATUS) {
                    // Read the initial state of the D+/D- pins. This allows us to detect Low-Speed
                    // devices before resetting the port.
                    if port_reg.get(PortStatusControl::LINE_STATUS) == LineStatus::KState {
                        /*
                         * The line being in K-state means the connected device is Low-Speed. It
                         * must be handed off to a companion controller.
                         */
                        trace!("Device on port {} is low-speed. Handing off to companion controller.", port);
                        unsafe {
                            self.write_port_register(
                                port,
                                PortStatusControl::new().with(PortStatusControl::PORT_OWNER, true),
                            );
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
            if self.read_port_register(port).get(PortStatusControl::PORT_ENABLED) {
                // The device is High-Speed. Let's manage it ourselves.
                let address = self.free_addresses.pop().unwrap();
                trace!("Device on port {} is high-speed. Allocated address {} for it to use.", port, address);

                // Create a new queue for the new device's control endpoint
                let mut queue = Queue::new(self.schedule_pool.create(QueueHead::new()).unwrap());
                self.add_to_async_schedule(&mut queue);

                /*
                 * People have found experientally that many devices, despite not being
                 * USB-compliant, expect the first request to unconditionally be of the max packet
                 * size. You can then set the device's address, then request the full descriptor
                 * like normal. For High-Speed devices, we do an initial request of 64 bytes.
                 * (see https://forum.osdev.org/viewtopic.php?f=1&t=56675&sid=817bd512e309859aed0ff09dc891cfcc&start=30)
                 *
                 * TODO: I'm not sure how correct any of this is on real hardware, as QEMU seems to
                 * accept pretty much anything. Apparently some devices also expect you to do a
                 * reset after requesting this first big packet. I think we'll need to test this
                 * out on real hardware once we have that up and running.
                 */
                let max_packet_size: u8 = {
                    let get_descriptor_header = SetupPacket {
                        typ: RequestType::new()
                            .with(RequestType::RECIPIENT, Recipient::Device)
                            .with(RequestType::TYP, RequestTypeType::Standard)
                            .with(RequestType::DIRECTION, Direction::DeviceToHost),
                        request: Request::GetDescriptor,
                        value: (DescriptorType::Device as u16) << 8,
                        index: 0,
                        length: 64,
                    };
                    let mut buffer = self.schedule_pool.create_buffer(64).unwrap();
                    queue.control_transfer(
                        get_descriptor_header,
                        Some(buffer.token().unwrap()),
                        false,
                        &mut self.schedule_pool,
                    );
                    self.wait_for_transfer_completion(&mut queue);

                    // Manually extract the max packet size from the buffer (one byte at `0x7`)
                    let max_packet_size = buffer.read()[7];
                    max_packet_size
                };
                info!("Max packet size: {}", max_packet_size);

                // TODO: apparently some devices expect you to reset them again after this?
                // TODO: set the max packet size

                /*
                 * Give the device an address.
                 */
                let set_address = SetupPacket {
                    typ: RequestType::new()
                        .with(RequestType::RECIPIENT, Recipient::Device)
                        .with(RequestType::TYP, RequestTypeType::Standard)
                        .with(RequestType::DIRECTION, Direction::HostToDevice),
                    request: Request::SetAddress,
                    value: address as u16,
                    index: 0,
                    length: 0,
                };
                queue.control_transfer(set_address, None, true, &mut self.schedule_pool);
                self.wait_for_transfer_completion(&mut queue);

                queue.set_address(address);

                // Get the rest of the descriptor
                let device_descriptor: DeviceDescriptor = {
                    let get_descriptor = SetupPacket {
                        typ: RequestType::new()
                            .with(RequestType::RECIPIENT, Recipient::Device)
                            .with(RequestType::TYP, RequestTypeType::Standard)
                            .with(RequestType::DIRECTION, Direction::DeviceToHost),
                        request: Request::GetDescriptor,
                        value: (DescriptorType::Device as u16) << 8,
                        index: 0,
                        length: mem::size_of::<DeviceDescriptor>() as u16,
                    };
                    let mut descriptor: DmaObject<DeviceDescriptor> =
                        self.schedule_pool.create(DeviceDescriptor::default()).unwrap();
                    queue.control_transfer(
                        get_descriptor,
                        Some(descriptor.token().unwrap()),
                        false,
                        &mut self.schedule_pool,
                    );
                    self.wait_for_transfer_completion(&mut queue);

                    *descriptor.read()
                };
                info!("Device Descriptor: {:#?}", device_descriptor);

                let configuration = {
                    /*
                     * A configuration is described by a Configuration descriptor, followed by
                     * other descriptors. We request the Configuration descriptor first, which
                     * contains the total size of the configuration's hierachy, and then request
                     * the whole thing in one go.
                     */
                    let get_descriptor = SetupPacket {
                        typ: RequestType::new()
                            .with(RequestType::RECIPIENT, Recipient::Device)
                            .with(RequestType::TYP, RequestTypeType::Standard)
                            .with(RequestType::DIRECTION, Direction::DeviceToHost),
                        request: Request::GetDescriptor,
                        value: (DescriptorType::Configuration as u16) << 8,
                        index: 0,
                        length: mem::size_of::<ConfigurationDescriptor>() as u16,
                    };
                    let mut descriptor: DmaObject<ConfigurationDescriptor> =
                        self.schedule_pool.create(ConfigurationDescriptor::default()).unwrap();
                    queue.control_transfer(
                        get_descriptor,
                        Some(descriptor.token().unwrap()),
                        false,
                        &mut self.schedule_pool,
                    );
                    self.wait_for_transfer_completion(&mut queue);

                    info!("ConfigurationDescriptor: {:#?}", descriptor.read());

                    let get_configuration = SetupPacket {
                        typ: RequestType::new()
                            .with(RequestType::RECIPIENT, Recipient::Device)
                            .with(RequestType::TYP, RequestTypeType::Standard)
                            .with(RequestType::DIRECTION, Direction::DeviceToHost),
                        request: Request::GetDescriptor,
                        value: (DescriptorType::Configuration as u16) << 8,
                        index: 0,
                        length: descriptor.read().total_length as u16,
                    };
                    let mut buffer =
                        self.schedule_pool.create_buffer(descriptor.read().total_length as usize).unwrap();
                    queue.control_transfer(
                        get_configuration,
                        Some(buffer.token().unwrap()),
                        false,
                        &mut self.schedule_pool,
                    );
                    self.wait_for_transfer_completion(&mut queue);

                    buffer.read().to_vec()
                };

                /*
                 * Add the newly discovered device to our list of active devices. This is important
                 * as it prevents the queue from being dropped while it is in the async schedule.
                 * TODO: might be safer to hold `Arc`s to the queue and keep track of stuff in the
                 * async schedule to stop queues being dropped out from under the controller?
                 */
                self.active_devices.insert(address, ActiveDevice { control_queue: queue });

                self.add_device_to_platform_bus(address, &device_descriptor, configuration);
            } else {
                /*
                 * The device is not High-Speed. Hand it off to a companion controller to deal
                 * with.
                 */
                trace!("Device on port {} is full-speed. Handing off to companion controller.", port);
                self.write_port_register(port, PortStatusControl::new().with(PortStatusControl::PORT_OWNER, true));
            }
        }
    }

    fn add_device_to_platform_bus(&mut self, address: u8, descriptor: &DeviceDescriptor, config0: Vec<u8>) {
        // TODO: when we've got hubs and stuff we'll need to keep track of bus numbers
        let bus = 0;
        let name = format!("usb-{}.{}", bus, address);
        let device_info = {
            let mut properties = BTreeMap::new();
            properties.insert("usb.device_class".to_string(), Property::Integer(descriptor.class as u64));
            properties.insert("usb.device_subclass".to_string(), Property::Integer(descriptor.sub_class as u64));
            properties.insert("usb.device_protocol".to_string(), Property::Integer(descriptor.protocol as u64));
            properties.insert("usb.device_vendor".to_string(), Property::Integer(descriptor.vendor_id as u64));
            properties.insert("usb.device_product".to_string(), Property::Integer(descriptor.product_id as u64));
            // TODO: we should probs include all the configurations to choose from no?
            // Maybe need a list, or just to append numbers idk?
            properties.insert("usb.config0".to_string(), Property::Bytes(config0));
            DeviceInfo(properties)
        };
        let (device_channel, device_channel_handle) =
            Channel::<(), DeviceControlMessage>::create().unwrap();
        let handoff_info = {
            let mut properties = BTreeMap::new();
            properties.insert("usb.channel".to_string(), HandoffProperty::Channel(device_channel_handle));
            HandoffInfo(properties)
        };
        self.platform_bus_bus_channel
            .send(&BusDriverMessage::RegisterDevice(name, device_info, handoff_info))
            .unwrap();

        std::poplar::rt::spawn(async move {
            loop {
                let message = device_channel.receive().await.unwrap();
                info!("Message down device channel: {:?}", message);
            }
        });
    }

    pub fn add_to_async_schedule(&mut self, queue: &mut Queue) {
        /*
         * TODO: this currently assumes we only have a single queue head. To manage more than one,
         * we need to:
         *    - Keep track of queues in the schedule (this will probably involve them become
         *      `Arc<RefCell<Queue>>`s is the problem)
         *    - If there are already queue heads in the schedule, point to the current head with
         *      our horizontal link ptr and then replace the `NextAsyncListAddress`.
         *    - Reconfigure the reclaim list heads - this newest queue head becomes the reclaim
         *      head, and the current one has its H-bit cleared.
         */
        queue.set_reclaim_head(true);
        queue.head.write().horizontal_link = HorizontalLinkPtr::new(queue.head.phys as u32, 0b01, false);
        unsafe {
            self.write_operational_register(OpRegister::NextAsyncListAddress, queue.head.phys as u32);
            self.write_operational_register(
                OpRegister::Command,
                Command::new()
                    .with(Command::RUN, true)
                    .with(Command::ASYNC_SCHEDULE_ENABLE, true)
                    .with(Command::INTERRUPT_THRESHOLD, 0x08)
                    .bits(),
            );
        }
    }

    /*
     * TODO: this is temporary. In the future, `control_transfer` etc. will return futures that get
     * magically handled by the IRQ handler.
     *
     * Wait for a transaction to complete, and then pop it off the queue. This releases the DMA
     * objects needed for the transaction, including the token for the data object used. This
     * allows the result to be read from it, if relevant.
     */
    pub fn wait_for_transfer_completion(&mut self, queue: &mut Queue) {
        std::poplar::syscall::wait_for_event(self.interrupt_event).unwrap();

        assert!(self.read_status().get(Status::INTERRUPT));
        info!("Transaction complete!");

        unsafe { self.write_status(Status::new().with(Status::INTERRUPT, true)) };

        // Remove the front transaction from the queue to drop the held DMA objects and token
        queue.transactions.pop_front();
    }

    pub fn reset_port(&mut self, port: u8) {
        unsafe {
            /*
             * Reset the port by toggling the PortReset bit and then waiting for it to clear.
             */
            self.write_port_register(port, PortStatusControl::new().with(PortStatusControl::PORT_RESET, true));
            // TODO: apparently we're meant to time a duration here???? QEMU doesn't complain about
            // no delay but I bet real ones do
            self.write_port_register(port, PortStatusControl::new());
            while self.read_port_register(port).get(PortStatusControl::PORT_RESET) {}
        }
    }

    pub fn read_status(&self) -> Status {
        Status::from_bits(unsafe { self.read_operational_register(OpRegister::Status) })
    }

    pub unsafe fn write_status(&mut self, value: Status) {
        unsafe {
            self.write_operational_register(OpRegister::Status, value.bits());
        }
    }

    pub fn read_command(&self) -> Command {
        Command::from_bits(unsafe { self.read_operational_register(OpRegister::Command) })
    }

    pub unsafe fn write_command(&mut self, value: Command) {
        unsafe {
            self.write_operational_register(OpRegister::Command, value.bits());
        }
    }

    pub unsafe fn read_operational_register(&self, reg: OpRegister) -> u32 {
        let address = self.register_base + self.caps.cap_length as usize + (reg as u32 as usize);
        unsafe { std::ptr::read_volatile(address as *mut u32) }
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
        PortStatusControl::from_bits(unsafe { std::ptr::read_volatile(address as *const u32) })
    }

    pub unsafe fn write_port_register(&self, port: u8, value: PortStatusControl) {
        let address =
            self.register_base + self.caps.cap_length as usize + 0x44 + mem::size_of::<u32>() * port as usize;
        unsafe {
            std::ptr::write_volatile(address as *mut u32, value.bits());
        }
    }
}

fn main() {
    log::set_logger(&EarlyLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("EHCI USB Bus Driver is running!");

    std::poplar::rt::init_runtime();

    // This allows us to talk to the PlatformBus as a bus driver (to register USB devices).
    let platform_bus_bus_channel: Arc<Channel<BusDriverMessage, !>> =
        Arc::new(Channel::subscribe_to_service("platform_bus.bus_driver").unwrap());
    // This allows us to talk to the PlatformBus as a device driver (to find controllers we can manage).
    let platform_bus_device_channel: Channel<DeviceDriverMessage, DeviceDriverRequest> =
        Channel::subscribe_to_service("platform_bus.device_driver").unwrap();

    // Tell PlatformBus that we're interested in EHCI controllers.
    platform_bus_device_channel
        .send(&DeviceDriverMessage::RegisterInterest(vec![
            Filter::Matches(String::from("pci.class"), Property::Integer(0x0c)),
            Filter::Matches(String::from("pci.sub_class"), Property::Integer(0x03)),
            Filter::Matches(String::from("pci.interface"), Property::Integer(0x20)),
        ]))
        .unwrap();

    // Spawn a task to listen for new controllers to drive
    std::poplar::rt::spawn(async move {
        loop {
            match platform_bus_device_channel.receive().await.unwrap() {
                DeviceDriverRequest::QuerySupport(device_name, _device_info) => {
                    /*
                     * Our filters are specific enough that any device that matches should be an
                     * EHCI controller, so we always say we'll support it here.
                     */
                    platform_bus_device_channel.send(&DeviceDriverMessage::CanSupport(device_name, true)).unwrap();
                }
                DeviceDriverRequest::HandoffDevice(device_name, device_info, handoff_info) => {
                    info!("Started driving a EHCI controller: {}", device_name);

                    let register_space_size = handoff_info.get_as_integer("pci.bar0.size").unwrap() as usize;

                    // TODO: let the kernel choose the address when it can - we don't care
                    // TODO: this trusts the data from the platform_bus. Maybe we shouldn't do that? One
                    // idea would be a syscall for querying info about the object?
                    let register_space = MemoryObject {
                        handle: handoff_info.get_as_memory_object("pci.bar0.handle").unwrap(),
                        size: register_space_size,
                        flags: MemoryObjectFlags::WRITABLE,
                        phys_address: None,
                    };
                    const REGISTER_SPACE_ADDRESS: usize = 0x00000005_00000000;
                    unsafe {
                        register_space.map_at(REGISTER_SPACE_ADDRESS).unwrap();
                    }

                    let mut controller = Controller::new(
                        REGISTER_SPACE_ADDRESS,
                        platform_bus_bus_channel.clone(),
                        handoff_info.get_as_event("pci.interrupt").unwrap(),
                    );
                    controller.initialize();
                    controller.check_ports();

                    // TODO: spawn task to listen for interrupts from the controller and respond to
                    // them
                }
            }
        }
    });

    std::poplar::rt::enter_loop();
}

#[used]
#[link_section = ".caps"]
pub static mut CAPS: CapabilitiesRepr<4> =
    CapabilitiesRepr::new([CAP_EARLY_LOGGING, CAP_SERVICE_USER, CAP_PADDING, CAP_PADDING]);
