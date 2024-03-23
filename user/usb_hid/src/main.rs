#![feature(never_type)]

use log::info;
use platform_bus::{BusDriverMessage, DeviceDriverMessage, DeviceDriverRequest, Filter, Property};
use std::poplar::{
    caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING, CAP_SERVICE_USER},
    channel::Channel,
    early_logger::EarlyLogger,
};
use usb::{descriptor::InterfaceDescriptor, DeviceControlMessage};

pub fn main() {
    log::set_logger(&EarlyLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("USB HID Driver is running!");

    std::poplar::rt::init_runtime();

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

    std::poplar::rt::spawn(async move {
        loop {
            match platform_bus_device_channel.receive().await.unwrap() {
                DeviceDriverRequest::QuerySupport(device_name, device_info) => {
                    info!(
                        "Platform bus asked if we can support device {} with info {:?}",
                        device_name, device_info
                    );
                    // TODO: consider each config if multiple?
                    let configuration = device_info.get_as_bytes("usb.config0").unwrap();
                    info!("USB config: {:x?}", configuration);

                    struct Visitor(pub bool);
                    impl usb::descriptor::ConfigurationVisitor for Visitor {
                        fn visit_interface(&mut self, descriptor: &InterfaceDescriptor) {
                            // Check if this interface indicates a HID class device
                            if descriptor.interface_class == 3 {
                                self.0 = true;
                            }
                        }
                    }

                    let supported = {
                        let mut visitor = Visitor(false);
                        usb::descriptor::walk_configuration(configuration, &mut visitor);
                        visitor.0
                    };
                    platform_bus_device_channel
                        .send(&DeviceDriverMessage::CanSupport(device_name, supported))
                        .unwrap();
                }
                DeviceDriverRequest::HandoffDevice(device_name, device_info, handoff_info) => {
                    info!("Started driving HID device '{}'", device_name);

                    // Test the device channel
                    let device_channel: Channel<DeviceControlMessage, ()> =
                        Channel::new_from_handle(handoff_info.get_as_channel("usb.channel").unwrap());
                    device_channel.send(&DeviceControlMessage::UseConfiguration(0)).unwrap();

                    // TODO: do something with the device
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
