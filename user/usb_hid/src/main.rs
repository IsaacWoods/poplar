#![feature(never_type)]

use log::info;
use platform_bus::{BusDriverMessage, DeviceDriverMessage, DeviceDriverRequest, Filter, Property};
use std::poplar::{
    caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING, CAP_SERVICE_USER},
    channel::Channel,
    early_logger::EarlyLogger,
};
use usb::descriptor::InterfaceDescriptor;

pub fn main() {
    log::set_logger(&EarlyLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("USB HID Driver is running!");

    // This allows us to talk to the PlatformBus as a bus driver (to register our abstract devices).
    let platform_bus_bus_channel: Channel<BusDriverMessage, !> =
        Channel::subscribe_to_service("platform_bus.bus_driver").unwrap();
    // This allows us to talk to the PlatformBus as a device driver (to find supported USB devices).
    let platform_bus_device_channel: Channel<DeviceDriverMessage, DeviceDriverRequest> =
        Channel::subscribe_to_service("platform_bus.device_driver").unwrap();

    // Tell PlatformBus that we're interested in USB devices that are specified per-interface
    // (we need to parse their configurations to tell if they're HID devices). A HID device is not
    // supposed to indicate its class at the device level so we don't need to test for that.
    platform_bus_device_channel
        .send(&DeviceDriverMessage::RegisterInterest(vec![
            Filter::Matches(String::from("usb.device_class"), Property::Integer(0x00)),
            Filter::Matches(String::from("usb.device_subclass"), Property::Integer(0x00)),
        ]))
        .unwrap();

    // TODO: we need to be able to support multiple devices, so this needs to be spawned as a task
    // that loops round listening permanently
    let (_device_info, _handoff_info) = loop {
        match platform_bus_device_channel.try_receive().unwrap() {
            Some(DeviceDriverRequest::QuerySupport(device_name, device_info)) => {
                info!("Platform bus asked if we can support device {} with info {:?}", device_name, device_info);
                // TODO: consider each config if multiple?
                let configuration = device_info.get_as_bytes("usb.config0").unwrap();
                info!("USB config: {:x?}", configuration);

                struct Visitor(pub bool);
                impl usb::ConfigurationVisitor for Visitor {
                    fn visit_interface(&mut self, descriptor: &InterfaceDescriptor) {
                        // Check if this interface indicates a HID class device
                        if descriptor.interface_class == 3 {
                            info!("Found a HID class interface!");
                            self.0 = true;
                        }
                    }
                }

                let supported = {
                    let mut visitor = Visitor(false);
                    usb::walk_configuration(configuration, &mut visitor);
                    visitor.0
                };
                platform_bus_device_channel
                    .send(&DeviceDriverMessage::CanSupport(device_name, supported))
                    .unwrap();
            }
            Some(DeviceDriverRequest::HandoffDevice(device_name, device_info, handoff_info)) => {
                info!("Starting driving HID device '{}'", device_name);
                break (device_info, handoff_info);
            }
            None => std::poplar::syscall::yield_to_kernel(),
        }
    };

    loop {
        std::poplar::syscall::yield_to_kernel();
    }
}

#[used]
#[link_section = ".caps"]
pub static mut CAPS: CapabilitiesRepr<4> =
    CapabilitiesRepr::new([CAP_EARLY_LOGGING, CAP_SERVICE_USER, CAP_PADDING, CAP_PADDING]);
