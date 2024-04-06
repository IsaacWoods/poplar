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
    ops::DerefMut,
    poplar::{
        caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING, CAP_SERVICE_USER},
        channel::Channel,
        ddk::dma::DmaPool,
        early_logger::EarlyLogger,
        memory_object::MemoryObject,
        syscall::MemoryObjectFlags,
    },
    sync::Arc,
};
use usb::{
    setup::{Direction, Recipient, Request, RequestType, RequestTypeType, SetupPacket},
    DeviceControlMessage,
};

pub struct ActiveDevice {
    pub address: u8,
    control_queue: Arc<RwSpinlock<Queue>>,
    channel: Channel<(), DeviceControlMessage>,
}

impl ActiveDevice {
    pub fn handle_request(
        &mut self,
        request: DeviceControlMessage,
        controller: &mut Controller,
    ) -> Result<(), ()> {
        match request {
            DeviceControlMessage::UseConfiguration(config) => {
                todo!();
            }
            DeviceControlMessage::UseInterface(interface, setting) => todo!(),
            DeviceControlMessage::OpenEndpoint(endpoint) => todo!(),
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

                    let mut controller = Arc::new(RwSpinlock::new(Controller::new(
                        REGISTER_SPACE_ADDRESS,
                        platform_bus_bus_channel.clone(),
                        handoff_info.get_as_event("pci.interrupt").unwrap(),
                    )));
                    controller.write().initialize();

                    let new_devices = controller.write().check_ports();
                    for device in new_devices {
                        let controller = controller.clone();
                        std::poplar::rt::spawn(async move {
                            loop {
                                let mut device = device.write();
                                let message = device.channel.receive().await.unwrap();
                                info!("Message down device channel: {:?}", message);
                                device.handle_request(message, controller.write().deref_mut()).unwrap();
                            }
                        });
                    }

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
