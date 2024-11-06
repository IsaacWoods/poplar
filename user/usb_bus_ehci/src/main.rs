//! `usb_bus_ehci` is a driver compatible with EHCI USB host controllers.
//!
//! ### Development
//!    - On QEMU, enabling tracing of `usb_ehci_*` events is helpful for debugging.

#![feature(never_type)]

mod caps;
mod controller;
mod queue;
mod reg;

use crate::queue::Queue;
use controller::Controller;
use log::info;
use platform_bus::{BusDriverMessage, DeviceDriverMessage, DeviceDriverRequest, Filter, Property};
use spinning_top::RwSpinlock;
use std::{
    collections::BTreeMap,
    ops::Deref,
    poplar::{
        channel::Channel,
        early_logger::EarlyLogger,
        memory_object::MemoryObject,
        syscall::MemoryObjectFlags,
    },
    sync::Arc,
};
use usb::{
    setup::{Direction, Recipient, Request, RequestType, RequestTypeType, SetupPacket},
    DeviceControlMessage,
    DeviceResponse,
    EndpointDirection,
};

pub struct ActiveDevice {
    pub address: u8,
    control_queue: Arc<RwSpinlock<Queue>>,
    endpoints: BTreeMap<u8, Arc<RwSpinlock<Queue>>>,
    channel: Channel<DeviceResponse, DeviceControlMessage>,
}

impl ActiveDevice {
    pub async fn handle_request(
        &mut self,
        request: DeviceControlMessage,
        controller: &Controller,
    ) -> Result<(), ()> {
        match request {
            DeviceControlMessage::UseConfiguration(config) => {
                // TODO: make sure the config number is valid (i.e. device has that many configs)

                let set_configuration = SetupPacket {
                    typ: RequestType::new()
                        .with(RequestType::RECIPIENT, Recipient::Device)
                        .with(RequestType::TYP, RequestTypeType::Standard)
                        .with(RequestType::DIRECTION, Direction::HostToDevice),
                    request: Request::SetConfiguration,
                    value: config as u16,
                    index: 0,
                    length: 0,
                };
                controller.do_control_transfer(&self.control_queue, set_configuration, None, true).await;

                Ok(())
            }
            DeviceControlMessage::UseInterface(interface, setting) => {
                let set_configuration = SetupPacket {
                    typ: RequestType::new()
                        .with(RequestType::RECIPIENT, Recipient::Device)
                        .with(RequestType::TYP, RequestTypeType::Standard)
                        .with(RequestType::DIRECTION, Direction::HostToDevice),
                    request: Request::SetInterface,
                    value: setting as u16,
                    index: interface as u16,
                    length: 0,
                };
                controller.do_control_transfer(&self.control_queue, set_configuration, None, true).await;

                Ok(())
            }
            DeviceControlMessage::OpenEndpoint { number, direction, max_packet_size } => {
                match direction {
                    EndpointDirection::In => {
                        info!(
                            "Setting up IN pipe for endpoint {} (max packet size of {})",
                            number, max_packet_size
                        );

                        let queue = controller.create_queue(self.address, number, max_packet_size);
                        // TODO: I think in the long run things like Interrupt endpoints should
                        // actually be in the periodic schedule no?
                        controller.add_to_async_schedule(queue.clone());
                        self.endpoints.insert(number, queue);
                    }
                    EndpointDirection::Out => {
                        info!(
                            "Setting up OUT pipe for endpoint {} (max packet size of {})",
                            number, max_packet_size
                        );
                        todo!()
                    }
                }

                Ok(())
            }
            DeviceControlMessage::GetInterfaceDescriptor { typ, index, length } => {
                let get_descriptor = SetupPacket {
                    typ: RequestType::new()
                        .with(RequestType::RECIPIENT, Recipient::Interface)
                        .with(RequestType::TYP, RequestTypeType::Standard)
                        .with(RequestType::DIRECTION, Direction::DeviceToHost),
                    request: Request::GetDescriptor,
                    value: (typ as u16) << 8 + index,
                    index: 0,
                    length,
                };
                let mut buffer = controller.schedule_pool.write().create_buffer(length as usize).unwrap();
                controller
                    .do_control_transfer(&self.control_queue, get_descriptor, Some(buffer.token().unwrap()), false)
                    .await;

                self.channel
                    .send(&DeviceResponse::Descriptor { typ, index, bytes: buffer.read().to_vec() })
                    .unwrap();
                Ok(())
            }
            DeviceControlMessage::InterruptTransferIn { endpoint, packet_size } => {
                // info!("Doing IN interrupt transfer for endpoint {} (packet size = {})", endpoint, packet_size);
                let endpoint = self.endpoints.get(&endpoint).unwrap();
                // TODO: check that given direction is correct for this endpoint

                let mut buffer = controller.schedule_pool.write().create_buffer(packet_size as usize).unwrap();
                controller.do_interrupt_transfer(&endpoint, buffer.token().unwrap(), false).await;
                // TODO: I wonder if sending the data back should be divorced from the request
                // handling so we can handle other requests while we're waiting for it to complete?
                // This will require transactions to go through the async system first.
                self.channel.send(&DeviceResponse::Data(buffer.read().to_vec())).unwrap();
                Ok(())
            }
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

                    let controller = Controller::new(
                        REGISTER_SPACE_ADDRESS,
                        platform_bus_bus_channel.clone(),
                        handoff_info.get_as_event("pci.interrupt").unwrap(),
                    );
                    controller.initialize();

                    let new_devices = controller.check_ports().await;
                    for device in new_devices {
                        let controller = controller.clone();
                        std::poplar::rt::spawn(async move {
                            loop {
                                let mut device = device.write();
                                let message = device.channel.receive().await.unwrap();
                                device.handle_request(message, controller.deref()).await.unwrap();
                            }
                        });
                    }
                }
            }
        }
    });

    std::poplar::rt::enter_loop();
}
