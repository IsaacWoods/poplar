#![feature(never_type)]

mod caps;

use crate::caps::Capabilities;
use log::info;
use platform_bus::{BusDriverMessage, DeviceDriverMessage, DeviceDriverRequest, Filter, Property};
use std::poplar::{
    caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING, CAP_SERVICE_USER},
    channel::Channel,
    early_logger::EarlyLogger,
    syscall,
};

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

    let register_space_size = controller_device.properties.get("pci.bar0.size").unwrap().as_integer().unwrap();
    const REGISTER_SPACE_ADDRESS: usize = 0x00000005_00000000;
    unsafe {
        syscall::map_memory_object(
            controller_device.properties.get("pci.bar0.handle").as_ref().unwrap().as_memory_object().unwrap(),
            &std::poplar::ZERO_HANDLE,
            Some(REGISTER_SPACE_ADDRESS),
            0x0 as *mut usize,
        )
        .unwrap();
    }

    let capabilities = unsafe { Capabilities::read_from_registers(REGISTER_SPACE_ADDRESS) };
    info!("Capabilites: {:#?}", capabilities);

    loop {
        std::poplar::syscall::yield_to_kernel();
    }
}

#[used]
#[link_section = ".caps"]
pub static mut CAPS: CapabilitiesRepr<4> =
    CapabilitiesRepr::new([CAP_EARLY_LOGGING, CAP_SERVICE_USER, CAP_PADDING, CAP_PADDING]);
