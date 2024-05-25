#![feature(never_type)]

use log::{info, warn};
use platform_bus::{
    hid::{HidEvent, KeyState},
    BusDriverMessage,
    DeviceDriverMessage,
    DeviceDriverRequest,
    DeviceInfo,
    Filter,
    HandoffInfo,
    HandoffProperty,
    Property,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    poplar::{
        caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_PADDING, CAP_SERVICE_USER},
        channel::Channel,
        early_logger::EarlyLogger,
    },
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
            Filter::Matches(String::from("usb.class"), Property::Integer(0x00)),
            Filter::Matches(String::from("usb.sub_class"), Property::Integer(0x00)),
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

                    let control_channel: Channel<DeviceControlMessage, DeviceResponse> =
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

                    /*
                     * Register the device as a abstract HID device on the Platform Bus.
                     * TODO: we need to work out what devices actually are don't we...
                     */
                    let (device_channel, device_channel_other_end) = Channel::<HidEvent, ()>::create().unwrap();
                    let name = "usb-hid".to_string(); // TODO: proper name
                    let device_info = {
                        let mut info = BTreeMap::new();
                        info.insert("hid.type".to_string(), Property::String("keyboard".to_string()));
                        DeviceInfo(info)
                    };
                    let handoff_info = {
                        let mut info = BTreeMap::new();
                        info.insert("hid.channel".to_string(), HandoffProperty::Channel(device_channel_other_end));
                        HandoffInfo(info)
                    };
                    platform_bus_bus_channel
                        .send(&BusDriverMessage::RegisterDevice(name, device_info, handoff_info))
                        .unwrap();

                    std::poplar::rt::spawn(async move {
                        // Get the report descriptor
                        control_channel
                            .send(&DeviceControlMessage::GetInterfaceDescriptor {
                                typ: DescriptorType::Report,
                                index: 0,
                                length: config_info.hid_report_len,
                            })
                            .unwrap();
                        let report_desc = {
                            let bytes = match control_channel.receive().await.unwrap() {
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

                        control_channel
                            .send(&DeviceControlMessage::UseConfiguration(config_info.config_value))
                            .unwrap();
                        control_channel
                            .send(&DeviceControlMessage::OpenEndpoint {
                                number: config_info.endpoint_num,
                                direction: EndpointDirection::In,
                                max_packet_size: config_info.packet_size,
                            })
                            .unwrap();

                        /*
                         * This tracks the keys that are currently pressed, and how many polling
                         * cycles each has been pressed for. This is at the heart of the driver's
                         * ability to debounce key presses and then re-add key repetition in
                         * software.
                         * TODO: this currently just polls as-fast-as-it-can. We probably want to
                         * not do that, so add timing or move to the periodic schedule and do it
                         * properly.
                         * TODO: some drivers debounce keys that are only pressed for e.g. a few
                         * ms. I don't know if that's needed for real hardware, but something to
                         * consider (esp if we ever get spurious key presses).
                         * TODO: we don't currently do key repetition, as this requires accurate
                         * timing of each cycle.
                         */
                        let mut pressed_keys = BTreeMap::<Usage, u8>::new();

                        info!("Listening to reports from HID device '{}'", device_name);
                        loop {
                            control_channel
                                .send(&DeviceControlMessage::InterruptTransferIn {
                                    endpoint: config_info.endpoint_num,
                                    packet_size: config_info.packet_size,
                                })
                                .unwrap();
                            let response = control_channel.receive().await.unwrap();
                            match response {
                                DeviceResponse::Data(data) => {
                                    let report = report_desc.interpret(&data);
                                    let mut state = KeyState::default();
                                    let mut current_keys = BTreeSet::new();

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
                                                current_keys.insert(usage);
                                            }
                                            FieldValue::Selector(None) => {}
                                        }
                                    }

                                    pressed_keys = pressed_keys
                                        .into_iter()
                                        .filter_map(|(usage, count)| {
                                            if current_keys.take(&usage).is_some() {
                                                Some((usage, count + 1))
                                            } else {
                                                device_channel
                                                    .send(&HidEvent::KeyReleased {
                                                        key: map_usage(usage, state).unwrap(),
                                                        state,
                                                    })
                                                    .unwrap();
                                                None
                                            }
                                        })
                                        .collect();
                                    for new_key in current_keys.into_iter() {
                                        pressed_keys.insert(new_key, 1);
                                        device_channel
                                            .send(&HidEvent::KeyPressed {
                                                key: map_usage(new_key, state).unwrap(),
                                                state,
                                            })
                                            .unwrap();
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

// TODO: we should probably be able to define a keymap in a more data-oriented way in the future
// TODO: I'm not sure if we'll want to map everything to UTF-8 or if some would need different
// control-esque types or something?
fn map_usage(usage: Usage, state: KeyState) -> Option<char> {
    match (usage, state.shift()) {
        (Usage::KeyA, false) => Some('a'),
        (Usage::KeyA, true) => Some('A'),
        (Usage::KeyB, false) => Some('b'),
        (Usage::KeyB, true) => Some('B'),
        (Usage::KeyC, false) => Some('c'),
        (Usage::KeyC, true) => Some('C'),
        (Usage::KeyD, false) => Some('d'),
        (Usage::KeyD, true) => Some('D'),
        (Usage::KeyE, false) => Some('e'),
        (Usage::KeyE, true) => Some('E'),
        (Usage::KeyF, false) => Some('f'),
        (Usage::KeyF, true) => Some('F'),
        (Usage::KeyG, false) => Some('g'),
        (Usage::KeyG, true) => Some('G'),
        (Usage::KeyH, false) => Some('h'),
        (Usage::KeyH, true) => Some('H'),
        (Usage::KeyI, false) => Some('i'),
        (Usage::KeyI, true) => Some('I'),
        (Usage::KeyJ, false) => Some('j'),
        (Usage::KeyJ, true) => Some('J'),
        (Usage::KeyK, false) => Some('k'),
        (Usage::KeyK, true) => Some('K'),
        (Usage::KeyL, false) => Some('l'),
        (Usage::KeyL, true) => Some('L'),
        (Usage::KeyM, false) => Some('m'),
        (Usage::KeyM, true) => Some('M'),
        (Usage::KeyN, false) => Some('n'),
        (Usage::KeyN, true) => Some('N'),
        (Usage::KeyO, false) => Some('o'),
        (Usage::KeyO, true) => Some('O'),
        (Usage::KeyP, false) => Some('p'),
        (Usage::KeyP, true) => Some('P'),
        (Usage::KeyQ, false) => Some('q'),
        (Usage::KeyQ, true) => Some('Q'),
        (Usage::KeyR, false) => Some('r'),
        (Usage::KeyR, true) => Some('R'),
        (Usage::KeyS, false) => Some('s'),
        (Usage::KeyS, true) => Some('S'),
        (Usage::KeyT, false) => Some('t'),
        (Usage::KeyT, true) => Some('T'),
        (Usage::KeyU, false) => Some('u'),
        (Usage::KeyU, true) => Some('U'),
        (Usage::KeyV, false) => Some('v'),
        (Usage::KeyV, true) => Some('V'),
        (Usage::KeyW, false) => Some('w'),
        (Usage::KeyW, true) => Some('W'),
        (Usage::KeyX, false) => Some('x'),
        (Usage::KeyX, true) => Some('X'),
        (Usage::KeyY, false) => Some('y'),
        (Usage::KeyY, true) => Some('Y'),
        (Usage::KeyZ, false) => Some('z'),
        (Usage::Key1, false) => Some('1'),
        (Usage::Key1, true) => Some('!'),
        (Usage::Key2, false) => Some('2'),
        (Usage::Key2, true) => Some('@'),
        (Usage::Key3, false) => Some('3'),
        (Usage::Key3, true) => Some('#'),
        (Usage::Key4, false) => Some('4'),
        (Usage::Key4, true) => Some('$'),
        (Usage::Key5, false) => Some('5'),
        (Usage::Key5, true) => Some('%'),
        (Usage::Key6, false) => Some('6'),
        (Usage::Key6, true) => Some('^'),
        (Usage::Key7, false) => Some('7'),
        (Usage::Key7, true) => Some('&'),
        (Usage::Key8, false) => Some('8'),
        (Usage::Key8, true) => Some('*'),
        (Usage::Key9, false) => Some('9'),
        (Usage::Key9, true) => Some('('),
        (Usage::Key0, false) => Some('0'),
        (Usage::Key0, true) => Some(')'),
        (Usage::KeyReturn, _) => Some('\n'),
        (Usage::KeyEscape, _) => None,
        /*
         * XXX: confusingly, `KeyDelete` is actually backspace, and delete is `KeyDeleteForward`.
         * We then send a `0x7f` ASCII `DEL`, which differs from an ASCII backspace (`0x08`), which
         * moves the cursor but does not delete a character.
         */
        (Usage::KeyDelete, _) => Some('\x7f'),
        (Usage::KeyTab, _) => Some('\t'),
        (Usage::KeySpace, _) => Some(' '),
        (Usage::KeyDash, false) => Some('-'),
        (Usage::KeyDash, true) => Some('_'),
        (Usage::KeyEquals, false) => Some('='),
        (Usage::KeyEquals, true) => Some('+'),
        (Usage::KeyLeftBracket, false) => Some('['),
        (Usage::KeyLeftBracket, true) => Some('{'),
        (Usage::KeyRightBracket, false) => Some(']'),
        (Usage::KeyRightBracket, true) => Some('}'),
        (Usage::KeyForwardSlash, false) => Some('\\'),
        (Usage::KeyForwardSlash, true) => Some('|'),
        (Usage::KeyPound, _) => Some('#'),
        (Usage::KeySemicolon, false) => Some(';'),
        (Usage::KeySemicolon, true) => Some(':'),
        (Usage::KeyApostrophe, false) => Some('\''),
        (Usage::KeyApostrophe, true) => Some('"'),
        (Usage::KeyGrave, false) => Some('`'),
        (Usage::KeyGrave, true) => Some('~'),
        (Usage::KeyComma, false) => Some(','),
        (Usage::KeyComma, true) => Some('.'),
        (Usage::KeyDot, false) => Some('.'),
        (Usage::KeyBackSlash, false) => Some('/'),
        (Usage::KeyBackSlash, true) => Some('?'),
        _ => None,
    }
}

#[used]
#[link_section = ".caps"]
pub static mut CAPS: CapabilitiesRepr<4> =
    CapabilitiesRepr::new([CAP_EARLY_LOGGING, CAP_SERVICE_USER, CAP_PADDING, CAP_PADDING]);
