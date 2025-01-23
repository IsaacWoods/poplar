#![feature(never_type)]

use log::{info, warn};
use platform_bus::{
    input::{InputEvent, Key, KeyState},
    BusDriverMessage,
    DeviceDriverMessage,
    DeviceDriverRequest,
    DeviceInfo,
    Filter,
    HandoffInfo,
    HandoffProperty,
    Property,
};
use service_host::ServiceHostClient;
use std::{
    collections::{BTreeMap, BTreeSet},
    poplar::{channel::Channel, early_logger::EarlyLogger},
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

    let service_host_client = ServiceHostClient::new();
    // This allows us to talk to the PlatformBus as a bus driver (to register our abstract devices).
    let platform_bus_bus_channel: Channel<BusDriverMessage, !> =
        service_host_client.subscribe_service("platform_bus.bus_driver").unwrap();
    // This allows us to talk to the PlatformBus as a device driver (to find supported USB devices).
    let platform_bus_device_channel: Channel<DeviceDriverMessage, DeviceDriverRequest> =
        service_host_client.subscribe_service("platform_bus.device_driver").unwrap();

    // Tell PlatformBus that we're interested in USB devices that are specified per-interface
    // (we need to parse their configurations to tell if they're HID devices). A HID device is not
    // supposed to indicate its class at the device level so we don't need to test for that.
    platform_bus_device_channel
        .send(&DeviceDriverMessage::RegisterInterest(vec![Filter::All(vec![
            Filter::Matches(String::from("usb.class"), Property::Integer(0x00)),
            Filter::Matches(String::from("usb.sub_class"), Property::Integer(0x00)),
        ])]))
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
                            interface_protocol: u8,
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
                                self.interface_protocol = descriptor.interface_protocol;
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
                    let (device_channel, device_channel_other_end) = Channel::<InputEvent, ()>::create().unwrap();
                    // TODO: proper name
                    let name = "usb-hid".to_string();
                    // TODO: make this a proper enum I think?
                    let typ = match config_info.interface_protocol {
                        0 => "none",
                        1 => "keyboard",
                        2 => "mouse",
                        other => {
                            warn!("Reserved interface protocol in HID device descriptor: {}", other);
                            "reserved"
                        }
                    };
                    let device_info = {
                        let mut info = BTreeMap::new();
                        info.insert("hid.type".to_string(), Property::String(typ.to_string()));
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
                        info!("Parsed report descriptor: {:#?}", report_desc);

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
                                            // TODO: this filters out empty key entries - we should
                                            // probably do this in the report parsing code instead
                                            // of here?
                                            FieldValue::UntranslatedSelector { usage_page: 7, usage: 0x00 } => {}

                                            FieldValue::UntranslatedSelector { usage_page, usage } => {
                                                warn!("Received unknown selector in HID report: page={:#x}, usage={:#x}", usage_page, usage);
                                            }
                                            FieldValue::UntranslatedDynamicValue { usage_page, usage } => {
                                                warn!("Received unknown dynamic value in HID report: page={:#x}, usage={:#x}", usage_page, usage);
                                            }

                                            FieldValue::DynamicValue(Usage::X, value) => {
                                                if value != 0 {
                                                    device_channel.send(&InputEvent::RelX(value)).unwrap();
                                                }
                                            }
                                            FieldValue::DynamicValue(Usage::Y, value) => {
                                                if value != 0 {
                                                    device_channel.send(&InputEvent::RelY(value)).unwrap();
                                                }
                                            }
                                            FieldValue::DynamicValue(Usage::Z, value) => {
                                                if value != 0 {
                                                    device_channel.send(&InputEvent::RelZ(value)).unwrap();
                                                }
                                            }
                                            FieldValue::DynamicValue(Usage::Wheel, value) => {
                                                if value != 0 {
                                                    device_channel.send(&InputEvent::RelWheel(value)).unwrap();
                                                }
                                            }
                                            FieldValue::DynamicValue(
                                                usage @ (Usage::Button1
                                                | Usage::Button2
                                                | Usage::Button3
                                                | Usage::Button4
                                                | Usage::Button5),
                                                value,
                                            ) => {
                                                let map_button = |usage| match usage {
                                                    Usage::Button1 => Key::BtnLeft,
                                                    Usage::Button2 => Key::BtnRight,
                                                    Usage::Button3 => Key::BtnMiddle,
                                                    Usage::Button4 => Key::BtnSide,
                                                    Usage::Button5 => Key::BtnExtra,
                                                    _ => unreachable!(),
                                                };

                                                if value != 0 {
                                                    device_channel
                                                        .send(&InputEvent::KeyPressed {
                                                            key: map_button(usage),
                                                            state: KeyState::default(),
                                                        })
                                                        .unwrap();
                                                } else {
                                                    device_channel
                                                        .send(&InputEvent::KeyReleased {
                                                            key: map_button(usage),
                                                            state: KeyState::default(),
                                                        })
                                                        .unwrap();
                                                }
                                            }

                                            FieldValue::DynamicValue(Usage::KeyLeftControl, value) => {
                                                state.left_ctrl = value != 0;
                                            }
                                            FieldValue::DynamicValue(Usage::KeyLeftShift, value) => {
                                                state.left_shift = value != 0;
                                            }
                                            FieldValue::DynamicValue(Usage::KeyLeftAlt, value) => {
                                                state.left_alt = value != 0;
                                            }
                                            FieldValue::DynamicValue(Usage::KeyLeftGui, value) => {
                                                state.left_gui = value != 0;
                                            }
                                            FieldValue::DynamicValue(Usage::KeyRightControl, value) => {
                                                state.right_ctrl = value != 0;
                                            }
                                            FieldValue::DynamicValue(Usage::KeyRightShift, value) => {
                                                state.right_shift = value != 0;
                                            }
                                            FieldValue::DynamicValue(Usage::KeyRightAlt, value) => {
                                                state.right_alt = value != 0;
                                            }
                                            FieldValue::DynamicValue(Usage::KeyRightGui, value) => {
                                                state.right_gui = value != 0;
                                            }
                                            FieldValue::DynamicValue(other, _) => {
                                                warn!("Unknown dynamic flag: {:?}", other);
                                            }

                                            FieldValue::Selector(usage) => {
                                                current_keys.insert(usage);
                                            }
                                        }
                                    }

                                    pressed_keys = pressed_keys
                                        .into_iter()
                                        .filter_map(|(usage, count)| {
                                            if current_keys.take(&usage).is_some() {
                                                Some((usage, count + 1))
                                            } else {
                                                device_channel
                                                    .send(&InputEvent::KeyReleased {
                                                        key: map_key_usage(usage),
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
                                            .send(&InputEvent::KeyPressed { key: map_key_usage(new_key), state })
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

fn map_key_usage(usage: Usage) -> Key {
    match usage {
        Usage::KeyA => Key::KeyA,
        Usage::KeyB => Key::KeyB,
        Usage::KeyC => Key::KeyC,
        Usage::KeyD => Key::KeyD,
        Usage::KeyE => Key::KeyE,
        Usage::KeyF => Key::KeyF,
        Usage::KeyG => Key::KeyG,
        Usage::KeyH => Key::KeyH,
        Usage::KeyI => Key::KeyI,
        Usage::KeyJ => Key::KeyJ,
        Usage::KeyK => Key::KeyK,
        Usage::KeyL => Key::KeyL,
        Usage::KeyM => Key::KeyM,
        Usage::KeyN => Key::KeyN,
        Usage::KeyO => Key::KeyO,
        Usage::KeyP => Key::KeyP,
        Usage::KeyQ => Key::KeyQ,
        Usage::KeyR => Key::KeyR,
        Usage::KeyS => Key::KeyS,
        Usage::KeyT => Key::KeyT,
        Usage::KeyU => Key::KeyU,
        Usage::KeyV => Key::KeyV,
        Usage::KeyW => Key::KeyW,
        Usage::KeyX => Key::KeyX,
        Usage::KeyY => Key::KeyY,
        Usage::KeyZ => Key::KeyZ,
        Usage::Key1 => Key::Key1,
        Usage::Key2 => Key::Key2,
        Usage::Key3 => Key::Key3,
        Usage::Key4 => Key::Key4,
        Usage::Key5 => Key::Key5,
        Usage::Key6 => Key::Key6,
        Usage::Key7 => Key::Key7,
        Usage::Key8 => Key::Key8,
        Usage::Key9 => Key::Key9,
        Usage::Key0 => Key::Key0,
        Usage::KeyReturn => Key::KeyReturn,
        Usage::KeyEscape => Key::KeyEscape,
        Usage::KeyDelete => Key::KeyDelete,
        Usage::KeyTab => Key::KeyTab,
        Usage::KeySpace => Key::KeySpace,
        Usage::KeyDash => Key::KeyDash,
        Usage::KeyEquals => Key::KeyEquals,
        Usage::KeyLeftBracket => Key::KeyLeftBracket,
        Usage::KeyRightBracket => Key::KeyRightBracket,
        Usage::KeyForwardSlash => Key::KeyForwardSlash,
        Usage::KeyPound => Key::KeyPound,
        Usage::KeySemicolon => Key::KeySemicolon,
        Usage::KeyApostrophe => Key::KeyApostrophe,
        Usage::KeyGrave => Key::KeyGrave,
        Usage::KeyComma => Key::KeyComma,
        Usage::KeyDot => Key::KeyDot,
        Usage::KeyBackSlash => Key::KeyBackSlash,
        Usage::KeyCapslock => Key::KeyCapslock,
        Usage::KeyF1 => Key::KeyF1,
        Usage::KeyF2 => Key::KeyF2,
        Usage::KeyF3 => Key::KeyF3,
        Usage::KeyF4 => Key::KeyF4,
        Usage::KeyF5 => Key::KeyF5,
        Usage::KeyF6 => Key::KeyF6,
        Usage::KeyF7 => Key::KeyF7,
        Usage::KeyF8 => Key::KeyF8,
        Usage::KeyF9 => Key::KeyF9,
        Usage::KeyF10 => Key::KeyF10,
        Usage::KeyF11 => Key::KeyF11,
        Usage::KeyF12 => Key::KeyF12,
        Usage::KeyPrintScreen => Key::KeyPrintScreen,
        Usage::KeyScrolllock => Key::KeyScrolllock,
        Usage::KeyPause => Key::KeyPause,
        Usage::KeyInsert => Key::KeyInsert,
        Usage::KeyHome => Key::KeyHome,
        Usage::KeyPageUp => Key::KeyPageUp,
        Usage::KeyDeleteForward => Key::KeyDeleteForward,
        Usage::KeyEnd => Key::KeyEnd,
        Usage::KeyPageDown => Key::KeyPageDown,
        Usage::KeyRightArrow => Key::KeyRightArrow,
        Usage::KeyLeftArrow => Key::KeyLeftArrow,
        Usage::KeyDownArrow => Key::KeyDownArrow,
        Usage::KeyUpArrow => Key::KeyUpArrow,
        Usage::KeyNumlock => Key::KeyNumlock,
        Usage::KeypadSlash => Key::KeypadSlash,
        Usage::KeypadAsterix => Key::KeypadAsterix,
        Usage::KeypadDash => Key::KeypadDash,
        Usage::KeypadPlus => Key::KeypadPlus,
        Usage::KeypadEnter => Key::KeypadEnter,
        Usage::Keypad1 => Key::Keypad1,
        Usage::Keypad2 => Key::Keypad2,
        Usage::Keypad3 => Key::Keypad3,
        Usage::Keypad4 => Key::Keypad4,
        Usage::Keypad5 => Key::Keypad5,
        Usage::Keypad6 => Key::Keypad6,
        Usage::Keypad7 => Key::Keypad7,
        Usage::Keypad8 => Key::Keypad8,
        Usage::Keypad9 => Key::Keypad9,
        Usage::Keypad0 => Key::Keypad0,
        Usage::KeypadDot => Key::KeypadDot,
        Usage::KeypadNonUsBackSlash => Key::KeypadNonUsBackSlash,
        Usage::KeyApplication => Key::KeyApplication,
        Usage::KeyPower => Key::KeyPower,
        Usage::KeypadEquals => Key::KeypadEquals,
        Usage::KeyF13 => Key::KeyF13,
        Usage::KeyF14 => Key::KeyF14,
        Usage::KeyF15 => Key::KeyF15,
        Usage::KeyF16 => Key::KeyF16,
        Usage::KeyF17 => Key::KeyF17,
        Usage::KeyF18 => Key::KeyF18,
        Usage::KeyF19 => Key::KeyF19,
        Usage::KeyF20 => Key::KeyF20,
        Usage::KeyF21 => Key::KeyF21,
        Usage::KeyF22 => Key::KeyF22,
        Usage::KeyF23 => Key::KeyF23,
        Usage::KeyF24 => Key::KeyF24,
        Usage::KeyExecute => Key::KeyExecute,
        Usage::KeyHelp => Key::KeyHelp,
        Usage::KeyMenu => Key::KeyMenu,
        Usage::KeySelect => Key::KeySelect,
        Usage::KeyStop => Key::KeyStop,
        Usage::KeyAgain => Key::KeyAgain,
        Usage::KeyUndo => Key::KeyUndo,
        Usage::KeyCut => Key::KeyCut,
        Usage::KeyCopy => Key::KeyCopy,
        Usage::KeyPaste => Key::KeyPaste,
        Usage::KeyFind => Key::KeyFind,
        Usage::KeyMute => Key::KeyMute,
        Usage::KeyVolumeUp => Key::KeyVolumeUp,
        Usage::KeyVolumeDown => Key::KeyVolumeDown,
        Usage::KeyLockingCapslock => Key::KeyLockingCapslock,
        Usage::KeyLockingNumlock => Key::KeyLockingNumlock,
        Usage::KeyLockingScrolllock => Key::KeyLockingScrolllock,
        Usage::KeypadComma => Key::KeypadComma,
        Usage::KeyLeftControl => Key::KeyLeftControl,
        Usage::KeyLeftShift => Key::KeyLeftShift,
        Usage::KeyLeftAlt => Key::KeyLeftAlt,
        Usage::KeyLeftGui => Key::KeyLeftGui,
        Usage::KeyRightControl => Key::KeyRightControl,
        Usage::KeyRightShift => Key::KeyRightShift,
        Usage::KeyRightAlt => Key::KeyRightAlt,
        Usage::KeyRightGui => Key::KeyRightGui,
        _ => panic!("Unknown usage: {:?}", usage),
    }
}
