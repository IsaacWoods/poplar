#![feature(never_type)]

use log::{info, warn};
use platform_bus::{BusDriverMessage, DeviceDriverMessage, DeviceDriverRequest, Filter, Property};
use std::poplar::{
    caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING, CAP_SERVICE_USER},
    channel::Channel,
    early_logger::EarlyLogger,
};
use usb::{
    descriptor::{
        ConfigurationDescriptor,
        DescriptorType,
        EndpointAddress,
        EndpointDescriptor,
        InterfaceDescriptor,
    },
    hid::{
        report::{FieldValue, Usage},
        HidDescriptor,
    },
    DeviceControlMessage,
    DeviceResponse,
    EndpointDirection,
};

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

                    let device_channel: Channel<DeviceControlMessage, DeviceResponse> =
                        Channel::new_from_handle(handoff_info.get_as_channel("usb.channel").unwrap());

                    let config_info = {
                        // TODO: this assumes only one configuration
                        let bytes = device_info.get_as_bytes("usb.config0").unwrap();
                        #[derive(Default)]
                        struct ConfigInfo {
                            config_value: u8,
                            interface_num: u8,
                            interface_setting: u8,
                            endpoint_num: u8,
                            packet_size: u16,
                            hid_report_len: u16,
                        }
                        impl usb::descriptor::ConfigurationVisitor for ConfigInfo {
                            fn visit_configuration(&mut self, descriptor: &ConfigurationDescriptor) {
                                self.config_value = descriptor.configuration_value;
                            }

                            fn visit_interface(&mut self, descriptor: &InterfaceDescriptor) {
                                self.interface_num = descriptor.interface_num;
                                self.interface_setting = descriptor.alternate_setting;
                            }

                            fn visit_endpoint(&mut self, descriptor: &EndpointDescriptor) {
                                self.endpoint_num = descriptor.endpoint_address.get(EndpointAddress::NUMBER);
                                self.packet_size = descriptor.max_packet_size;
                            }

                            fn visit_hid(&mut self, descriptor: &HidDescriptor) {
                                // TODO: we might want to handle more descriptors than just the
                                // Report one (or it might not come first).
                                assert!(descriptor.descriptor_typ == 34);
                                self.hid_report_len = descriptor.descriptor_length;
                            }
                        }
                        let mut info = ConfigInfo::default();
                        usb::descriptor::walk_configuration(bytes, &mut info);
                        info
                    };

                    std::poplar::rt::spawn(async move {
                        // Get the report descriptor
                        device_channel
                            .send(&DeviceControlMessage::GetInterfaceDescriptor {
                                typ: DescriptorType::Report,
                                index: 0,
                                length: config_info.hid_report_len,
                            })
                            .unwrap();
                        let report_desc = {
                            let bytes = match device_channel.receive().await.unwrap() {
                                DeviceResponse::Descriptor { typ, index, bytes }
                                    if typ == DescriptorType::Report && index == 0 =>
                                {
                                    bytes
                                }
                                _ => panic!("Unexpected response from GetInterfaceDescriptor request!"),
                            };

                            info!("Got Report descriptor: {:x?}", bytes);
                            let report_desc = usb::hid::report::ReportDescriptorParser::parse(&bytes);
                            report_desc
                        };

                        device_channel
                            .send(&DeviceControlMessage::UseConfiguration(config_info.config_value))
                            .unwrap();
                        device_channel
                            .send(&DeviceControlMessage::OpenEndpoint {
                                number: config_info.endpoint_num,
                                direction: EndpointDirection::In,
                                max_packet_size: config_info.packet_size,
                            })
                            .unwrap();

                        info!("Listening to reports from HID device '{}'", device_name);
                        loop {
                            device_channel
                                .send(&DeviceControlMessage::InterruptTransferIn {
                                    endpoint: config_info.endpoint_num,
                                    packet_size: config_info.packet_size,
                                })
                                .unwrap();
                            let response = device_channel.receive().await.unwrap();
                            match response {
                                DeviceResponse::Data(data) => {
                                    let report = report_desc.interpret(&data);
                                    let mut state = KeyState::default();

                                    for field in report {
                                        match field {
                                            FieldValue::DynamicValue(Usage::KeyLeftControl, value) => {
                                                state.left_ctrl = value;
                                            }
                                            FieldValue::DynamicValue(Usage::KeyLeftShift, value) => {
                                                state.left_shift = value;
                                            }
                                            FieldValue::DynamicValue(Usage::KeyLeftAlt, value) => {
                                                state.left_alt = value;
                                            }
                                            FieldValue::DynamicValue(Usage::KeyLeftGui, value) => {
                                                state.left_gui = value;
                                            }
                                            FieldValue::DynamicValue(Usage::KeyRightControl, value) => {
                                                state.right_ctrl = value;
                                            }
                                            FieldValue::DynamicValue(Usage::KeyRightShift, value) => {
                                                state.right_shift = value;
                                            }
                                            FieldValue::DynamicValue(Usage::KeyRightAlt, value) => {
                                                state.right_alt = value;
                                            }
                                            FieldValue::DynamicValue(Usage::KeyRightGui, value) => {
                                                state.right_gui = value;
                                            }
                                            FieldValue::DynamicValue(other, _) => {
                                                warn!("Unknown dynamic flag: {:?}", other);
                                            }

                                            FieldValue::Selector(Some(usage)) => {
                                                // info!("Key pressed: {:?} ({:?})", usage, state);
                                                info!("Key pressed: {:?}", usage);
                                            }
                                            FieldValue::Selector(None) => {}
                                        }
                                    }
                                }
                                DeviceResponse::NoData => {}
                                _ => panic!("Unexpected message during report loop"),
                            }
                        }
                    });
                }
            }
        }
    });

    std::poplar::rt::enter_loop();
}

#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub struct KeyState {
    left_ctrl: bool,
    left_shift: bool,
    left_alt: bool,
    left_gui: bool,

    right_ctrl: bool,
    right_shift: bool,
    right_alt: bool,
    right_gui: bool,
}

impl KeyState {
    pub fn ctrl(&self) -> bool {
        self.left_ctrl || self.right_ctrl
    }

    pub fn shift(&self) -> bool {
        self.left_shift || self.right_shift
    }

    pub fn alt(&self) -> bool {
        self.left_alt || self.right_alt
    }

    pub fn gui(&self) -> bool {
        self.left_gui || self.right_gui
    }
}

#[used]
#[link_section = ".caps"]
pub static mut CAPS: CapabilitiesRepr<4> =
    CapabilitiesRepr::new([CAP_EARLY_LOGGING, CAP_SERVICE_USER, CAP_PADDING, CAP_PADDING]);
