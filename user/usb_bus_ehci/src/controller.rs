use crate::{
    caps::Capabilities,
    queue::{HorizontalLinkPtr, Queue, QueueHead},
    reg::{Command, InterruptEnable, LineStatus, OpRegister, PortStatusControl, RegisterBlock, Status},
    ActiveDevice,
};
use log::{info, trace};
use platform_bus::{BusDriverMessage, DeviceInfo, HandoffInfo, HandoffProperty, Property};
use spinning_top::RwSpinlock;
use std::{
    collections::BTreeMap,
    mem,
    ops::DerefMut,
    poplar::{
        channel::Channel,
        ddk::dma::{DmaObject, DmaPool, DmaToken},
        event::Event,
        memory_object::MemoryObject,
        syscall::MemoryObjectFlags,
    },
    sync::Arc,
};
use usb::{
    descriptor::{ConfigurationDescriptor, DescriptorType, DeviceDescriptor},
    setup::{Direction, Recipient, Request, RequestType, RequestTypeType, SetupPacket},
    DeviceControlMessage,
    DeviceResponse,
};

pub struct Controller {
    registers: RwSpinlock<RegisterBlock>,
    caps: Capabilities,
    free_addresses: Vec<u8>,
    pub schedule_pool: DmaPool,
    active_devices: BTreeMap<u8, Arc<RwSpinlock<ActiveDevice>>>,
    platform_bus_bus_channel: Arc<Channel<BusDriverMessage, !>>,
    interrupt_event: Event,

    /// Holds references to all the queues that are currently in the asynchronous schedule. It's
    /// important we keep a central record of them, as they are linked together into a linked list
    /// in physical memory; if a queue is dropped without removing it from the schedule, we'll
    /// confuse the controller.
    async_schedule: Vec<Arc<RwSpinlock<Queue>>>,
}

impl Controller {
    pub fn new(
        register_base: usize,
        platform_bus_bus_channel: Arc<Channel<BusDriverMessage, !>>,
        interrupt_event: Event,
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

        let registers = RwSpinlock::new(RegisterBlock::new(register_base, caps.cap_length));
        let controller = Arc::new(Controller {
            registers,
            caps,
            free_addresses: (1..128).collect(),
            schedule_pool,
            active_devices: BTreeMap::new(),
            platform_bus_bus_channel,
            interrupt_event,

            async_schedule: Vec::new(),
        }
    }

    pub fn initialize(&self) {
        /*
         * We only support controllers that don't support 64-bit addressing at the moment. This
         * means we don't need to set `CTRLDSSEGMENT`.
         */
        assert!(!self.caps.can_address_64bit);

        let mut registers = self.registers.write();

        // If the controller has already been used by the firmware, halt it before trying to reset
        if !registers.read_status().get(Status::CONTROLLER_HALTED) {
            info!("EHCI controller has already been started. Halting it.");
            let command = registers.read_command();
            unsafe {
                registers.write_command(
                    Command::new()
                        .with(Command::RUN, false)
                        .with(Command::INTERRUPT_THRESHOLD, command.get(Command::INTERRUPT_THRESHOLD)),
                );
            }
            while !registers.read_status().get(Status::CONTROLLER_HALTED) {}
        }

        // Reset the controller
        unsafe {
            registers
                .write_command(Command::new().with(Command::RESET, true).with(Command::INTERRUPT_THRESHOLD, 0x08));
            while registers.read_command().get(Command::RESET) {}
        }
        info!("EHCI controller reset");

        unsafe {
            // Enable interrupts we're interested in
            registers.write_operational_register(
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
            registers
                .write_command(Command::new().with(Command::RUN, true).with(Command::INTERRUPT_THRESHOLD, 0x08));

            // Route all ports to the EHCI controller
            registers.write_operational_register(OpRegister::ConfigFlag, 1);
        }
    }

    /// Iterate through the controller's connected ports, looking for device connects and
    /// disconnects. Each new device is added to the Platform Bus, and then a list of new devices
    /// is returned - the caller should ensure each device's channel is attended to so that
    /// requests from device drivers are handled.
    pub fn check_ports(&mut self) -> Vec<Arc<RwSpinlock<ActiveDevice>>> {
        assert!(!self.caps.port_power_control, "We don't support port power control");

        let mut new_devices = Vec::new();

        for port in 0..self.caps.num_ports {
            let port_reg = unsafe { self.registers.read().read_port_register(port) };

            if port_reg.get(PortStatusControl::PORT_ENABLED_CHANGE) {
                // TODO: handle this better
                info!("Port error on port {}", port);
            }

            if port_reg.get(PortStatusControl::CONNECT_STATUS_CHANGE) {
                // Clear the changed status by writing a `1` to it
                unsafe {
                    self.registers.write().write_port_register(
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
                            self.registers.write().write_port_register(
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
                        if let Some(new_device) = self.handle_device_connect(port) {
                            new_devices.push(new_device);
                        }
                    }
                } else {
                    trace!("Device on port {} disconnected", port);
                }
            }
        }

        new_devices
    }

    pub fn handle_device_connect(&mut self, port: u8) -> Option<Arc<RwSpinlock<ActiveDevice>>> {
        self.reset_port(port);

        unsafe {
            if self.registers.read().read_port_register(port).get(PortStatusControl::PORT_ENABLED) {
                // The device is High-Speed. Let's manage it ourselves.
                let address = self.free_addresses.pop().unwrap();
                trace!("Device on port {} is high-speed. Allocated address {} for it to use.", port, address);

                // Create a new queue for the new device's control endpoint
                let queue = self.create_queue(0, 0, 64);
                self.add_to_async_schedule(queue.clone());

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
                    self.do_control_transfer(&queue, get_descriptor_header, Some(buffer.token().unwrap()), false);

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
                self.do_control_transfer(&queue, set_address, None, true);

                queue.write().set_address(address);

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
                    self.do_control_transfer(&queue, get_descriptor, Some(descriptor.token().unwrap()), false);

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
                    self.do_control_transfer(&queue, get_descriptor, Some(descriptor.token().unwrap()), false);

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
                    self.do_control_transfer(&queue, get_configuration, Some(buffer.token().unwrap()), false);

                    buffer.read().to_vec()
                };

                let device = self.create_device(address, &device_descriptor, configuration, queue);
                Some(device)
            } else {
                /*
                 * The device is not High-Speed. Hand it off to a companion controller to deal
                 * with.
                 */
                trace!("Device on port {} is full-speed. Handing off to companion controller.", port);
                self.registers
                    .write()
                    .write_port_register(port, PortStatusControl::new().with(PortStatusControl::PORT_OWNER, true));
                None
            }
        }
    }

    fn create_device(
        &mut self,
        address: u8,
        descriptor: &DeviceDescriptor,
        config0: Vec<u8>,
        control_queue: Arc<RwSpinlock<Queue>>,
    ) -> Arc<RwSpinlock<ActiveDevice>> {
        /*
         * Create a Platform Bus device for this new device and advertise it to things that
         * might be interested in driving it.
         */
        // TODO: not sure whether this should be done in `Controller` or in the new-device-handling
        // code?
        // TODO: when we've got hubs and stuff we'll need to keep track of bus numbers
        let bus = 0;
        let name = format!("usb-{}.{}", bus, address);
        let device_info = {
            let mut properties = BTreeMap::new();
            properties.insert("usb.vendor_id".to_string(), Property::Integer(descriptor.vendor_id as u64));
            properties.insert("usb.product_id".to_string(), Property::Integer(descriptor.product_id as u64));
            properties.insert("usb.class".to_string(), Property::Integer(descriptor.class as u64));
            properties.insert("usb.sub_class".to_string(), Property::Integer(descriptor.sub_class as u64));
            properties.insert("usb.protocol".to_string(), Property::Integer(descriptor.protocol as u64));
            // TODO: we should probs include all the configurations to choose from no?
            // Maybe need a list, or just to append numbers idk?
            properties.insert("usb.config0".to_string(), Property::Bytes(config0));
            DeviceInfo(properties)
        };
        let (device_channel, device_channel_handle) =
            Channel::<DeviceResponse, DeviceControlMessage>::create().unwrap();
        let handoff_info = {
            let mut properties = BTreeMap::new();
            properties.insert("usb.channel".to_string(), HandoffProperty::Channel(device_channel_handle));
            HandoffInfo(properties)
        };
        self.platform_bus_bus_channel
            .send(&BusDriverMessage::RegisterDevice(name, device_info, handoff_info))
            .unwrap();

        let device = Arc::new(RwSpinlock::new(ActiveDevice {
            address,
            control_queue,
            endpoints: BTreeMap::new(),
            channel: device_channel,
        }));
        self.active_devices.insert(address, device.clone());

        device
    }

    pub fn add_to_async_schedule(&mut self, queue: Arc<RwSpinlock<Queue>>) {
        if self.async_schedule.is_empty() {
            /*
             * This is the first queue head being added to the schedule. We set it to loop back
             * round to itself, set it as the head of the reclamation list, and then set the async
             * schedule off running.
             *
             *     ┌─────────┐
             *     │  ┌───┐  │
             *     │  │ QH│  │
             *     └─►│  a├──┘
             *        └───┘RH
             *         ▲ ASYNCADDR
             */
            let mut locked_queue = queue.write();
            locked_queue.set_reclaim_head(true);
            locked_queue.head.write().horizontal_link =
                HorizontalLinkPtr::new(locked_queue.head.phys as u32, 0b01, false);
            unsafe {
                let mut registers = self.registers.write();
                registers
                    .write_operational_register(OpRegister::NextAsyncListAddress, locked_queue.head.phys as u32);
                registers.write_operational_register(
                    OpRegister::Command,
                    Command::new()
                        .with(Command::RUN, true)
                        .with(Command::ASYNC_SCHEDULE_ENABLE, true)
                        .with(Command::INTERRUPT_THRESHOLD, 0x08)
                        .bits(),
                );
            }
        } else {
            /*
             * There are already queue heads in the schedule. We want to add the new queue head
             * after the last element, and then link back round to the first. The newly added queue
             * head becomes the head of the reclamation list.
             *
             *     ┌─────────────────────────┐
             *     │  ┌───┐   ┌───┐   ┌───┐  │
             *     │  │ QH│   │ QH│   │ QH│  │
             *     └─►│  a├──►│  b├──►│  c├──┘
             *        └───┘   └───┘   └───┘RH
             *         ▲ ASYNCADDR
             */
            {
                let first = self.async_schedule.first().unwrap().read();
                let mut locked_queue = queue.write();
                locked_queue.head.write().horizontal_link =
                    HorizontalLinkPtr::new(first.head.phys as u32, 0b01, false);
                locked_queue.set_reclaim_head(true);
            }
            {
                let mut current_last = self.async_schedule.last_mut().unwrap().write();
                assert!(current_last.is_reclaim_head());
                current_last.head.write().horizontal_link =
                    HorizontalLinkPtr::new(queue.read().head.phys as u32, 0b01, false);
                current_last.set_reclaim_head(false);
            }
        }

        self.async_schedule.push(queue);
    }

    pub fn do_control_transfer(
        &mut self,
        queue: &Arc<RwSpinlock<Queue>>,
        setup: SetupPacket,
        data: Option<DmaToken>,
        transfer_to_device: bool,
    ) {
        let mut queue = queue.write();
        queue.control_transfer(setup, data, transfer_to_device, &mut self.schedule_pool);
        // TODO: this should be replaced with the async future thingy etc.
        self.wait_for_transfer_completion(queue.deref_mut());
    }

    pub fn do_interrupt_transfer(
        &mut self,
        queue: &Arc<RwSpinlock<Queue>>,
        data: DmaToken,
        transfer_to_device: bool,
    ) {
        let mut queue = queue.write();
        queue.interrupt_transfer(data, transfer_to_device, &mut self.schedule_pool);
        // TODO: this should be replaced with the async future thingy etc.
        self.wait_for_transfer_completion(queue.deref_mut());
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
        self.interrupt_event.wait_for_event_blocking();

        let status = self.read_status();
        if status.get(Status::ERR_INTERRUPT) {
            panic!("Transaction errored!");
        }

        assert!(status.get(Status::INTERRUPT));
        unsafe { self.write_status(Status::new().with(Status::INTERRUPT, true)) };

        // Remove the front transaction from the queue to drop the held DMA objects and token
        queue.transactions.pop_front();
    }

    pub fn reset_port(&self, port: u8) {
        unsafe {
            let registers = self.registers.write();

            /*
             * Reset the port by toggling the PortReset bit and then waiting for it to clear.
             */
            registers
                .write_port_register(port, PortStatusControl::new().with(PortStatusControl::PORT_RESET, true));
            // TODO: apparently we're meant to time a duration here???? QEMU doesn't complain about
            // no delay but I bet real ones do
            registers.write_port_register(port, PortStatusControl::new());
            while registers.read_port_register(port).get(PortStatusControl::PORT_RESET) {}
        }
    }

    pub fn create_queue(&mut self, device: u8, endpoint: u8, max_packet_size: u16) -> Arc<RwSpinlock<Queue>> {
        Arc::new(RwSpinlock::new(Queue::new(
            self.schedule_pool.create(QueueHead::new(device, endpoint, max_packet_size)).unwrap(),
            max_packet_size,
        )))
    }
}
