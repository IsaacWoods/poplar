//! `fb_console` is a console running on top of a framebuffer device, either provided through the
//! kernel or by a driver for a graphics-capable device.

// TODO: make a window manager and then make it so that this can drive a framebuffer directly, or
// create a window for itself.

use gfxconsole::{Format, Framebuffer, GfxConsole, Pixel, Rgb32};
use ginkgo::{
    ast::BindingResolver,
    interpreter::{Interpreter, Value},
    parse::Parser,
};
use log::info;
use platform_bus::{
    input::{InputEvent as PlatformBusInputEvent, Key, KeyState},
    DeviceDriverMessage,
    DeviceDriverRequest,
    Filter,
    Property,
};
use service_host::ServiceHostClient;
use spinning_top::Spinlock;
use std::{
    fmt::Write,
    poplar::{
        channel::Channel,
        early_logger::EarlyLogger,
        memory_object::{MappedMemoryObject, MemoryObject},
        syscall::MemoryObjectFlags,
    },
};

#[derive(Clone, Copy, Default, Debug)]
enum InputEvent {
    // TODO: it's unfortunate that this needs to exist
    #[default]
    Default,
    KeyPressed(char),
    RelX(i32),
    RelY(i32),
}

struct Console {
    framebuffer: MappedMemoryObject,
    control_channel: Channel<(), ()>,
    width: usize,
    height: usize,
    console: Spinlock<GfxConsole<Rgb32>>,
    input_events: thingbuf::mpsc::Receiver<InputEvent>,

    // TODO: we really need to separate out the like rendering/input management layer and the shell
    // logic
    platform_bus_inspect: Channel<(), platform_bus::PlatformBusInspect>,
}

fn spawn_framebuffer(
    framebuffer: MappedMemoryObject,
    channel: Channel<(), ()>,
    width: usize,
    height: usize,
    input_events: thingbuf::mpsc::Receiver<InputEvent>,
    service_host_client: &ServiceHostClient,
) {
    let platform_bus_inspect = service_host_client.subscribe_service("platform_bus.inspect").unwrap();

    let console = Spinlock::new(GfxConsole::new(
        Framebuffer { ptr: framebuffer.ptr() as *mut Pixel<Rgb32>, width, height, stride: width },
        Rgb32::pixel(0x00, 0x00, 0x00, 0x00),
        Rgb32::pixel(0xff, 0xff, 0xff, 0xff),
    ));
    let console = Console {
        framebuffer,
        control_channel: channel,
        width,
        height,
        console,
        input_events,
        platform_bus_inspect,
    };

    std::poplar::rt::spawn(async move {
        // TODO: separate out graphical layer and shell layer with another channel maybe??
        writeln!(console.console.lock(), "Welcome to Poplar!").unwrap();
        write!(console.console.lock(), "> ").unwrap();
        console.control_channel.send(&()).unwrap();

        let (output_sender, output_receiver) = thingbuf::mpsc::channel(16);

        let mut interpreter = Interpreter::new();
        let mut resolver = BindingResolver::new();
        let mut current_line = String::new();

        interpreter.define_native_function("print", |params| {
            assert!(params.len() == 1);
            let value = params.get(0).unwrap();
            output_sender.try_send(value.clone()).unwrap();
            Value::Unit
        });

        interpreter.define_native_function("version", |params| {
            assert!(params.len() == 0);
            /*
             * TODO: we don't really have a concept of Poplar versions yet. When this is more
             * formalised, we should get it from somewhere central (i.e. env var during build) so
             * this auto-updates.
             */
            Value::String("Poplar 0.1.0".to_string())
        });

        interpreter.define_native_function("inspect_platform_bus", |params| {
            assert!(params.len() == 0);
            console.platform_bus_inspect.send(&()).unwrap();
            let info = console.platform_bus_inspect.receive_blocking().unwrap();
            output_sender.try_send(Value::String(format!("{:#?}", info))).unwrap();
            Value::Bool(true)
        });

        let mut mouse_x = 300u32;
        let mut mouse_y = 300u32;

        loop {
            let mut needs_redraw = false;

            if let Some(event) = console.input_events.recv().await {
                match event {
                    InputEvent::KeyPressed(key) => {
                        // TODO: `noline` is a no-std REPL impl crate thingy that could be useful
                        // for improving this experience
                        match key {
                            '\n' => {
                                let mut stmts = Parser::new(&current_line).parse().unwrap();
                                current_line.clear();

                                for mut statement in &mut stmts {
                                    resolver.resolve_bindings(&mut statement);
                                }

                                let mut result = None;
                                for statement in stmts {
                                    match interpreter.eval_stmt(statement) {
                                        ginkgo::interpreter::ControlFlow::None => (),
                                        ginkgo::interpreter::ControlFlow::Yield(value) => {
                                            result = Some(value);
                                        }
                                        ginkgo::interpreter::ControlFlow::Return(value) => {
                                            result = Some(value);
                                        }
                                    }
                                }

                                write!(console.console.lock(), "{}", key).unwrap();
                                while let Ok(output) = output_receiver.try_recv() {
                                    writeln!(console.console.lock(), "Output: {}", output).unwrap();
                                }

                                if let Some(result) = result {
                                    writeln!(console.console.lock(), "Result: {}", result).unwrap();
                                }

                                write!(console.console.lock(), "\n> ").unwrap();
                                needs_redraw = true;
                            }

                            // ASCII `DEL` is produced by backspace
                            '\x7f' => {
                                // Only allow the user to delete characters they've typed.
                                if current_line.pop().is_some() {
                                    write!(console.console.lock(), "{}", key).unwrap();
                                    needs_redraw = true;
                                }
                            }

                            _ => {
                                write!(console.console.lock(), "{}", key).unwrap();
                                current_line.push(key);
                                needs_redraw = true;
                            }
                        }
                    }
                    InputEvent::RelX(value) => {
                        mouse_x = mouse_x.saturating_add_signed(value);
                        needs_redraw = true;
                    }
                    InputEvent::RelY(value) => {
                        mouse_y = mouse_y.saturating_add_signed(value);
                        needs_redraw = true;
                    }

                    InputEvent::Default => panic!(),
                }
            }

            if needs_redraw {
                // TODO: this obvs won't remove the old cursor - we need a proper thing for that...
                console.console.lock().framebuffer.draw_rect(
                    mouse_x as usize,
                    mouse_y as usize,
                    4,
                    4,
                    gfxconsole::Rgb32::pixel(0xff, 0, 0xff, 0xff),
                );
                console.control_channel.send(&()).unwrap();
            }
        }
    });
}

fn main() {
    log::set_logger(&EarlyLogger).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    info!("Framebuffer console is running!");

    std::poplar::rt::init_runtime();

    let (input_sender, input_receiver) = thingbuf::mpsc::channel(16);

    std::poplar::rt::spawn(async move {
        let mut input_receiver = Some(input_receiver);

        let service_host_client = ServiceHostClient::new();
        // We act as a device driver to find framebuffers and input devices
        let platform_bus_device_channel: Channel<DeviceDriverMessage, DeviceDriverRequest> =
            service_host_client.subscribe_service("platform_bus.device_driver").unwrap();
        platform_bus_device_channel
            .send(&DeviceDriverMessage::RegisterInterest(vec![
                Filter::Matches(String::from("type"), Property::String("framebuffer".to_string())),
                Filter::Matches(String::from("hid.type"), Property::String("keyboard".to_string())),
                Filter::Matches(String::from("hid.type"), Property::String("mouse".to_string())),
            ]))
            .unwrap();

        loop {
            let message = platform_bus_device_channel.receive().await.unwrap();
            match message {
                DeviceDriverRequest::QuerySupport(name, _) => {
                    platform_bus_device_channel.send(&DeviceDriverMessage::CanSupport(name, true)).unwrap();
                }
                DeviceDriverRequest::HandoffDevice(name, device_info, handoff_info) => {
                    if let Some("framebuffer") = device_info.get_as_str("type") {
                        info!("Found framebuffer device: {}", name);

                        let (width, height) = (
                            device_info.get_as_integer("width").unwrap() as usize,
                            device_info.get_as_integer("height").unwrap() as usize,
                        );
                        let framebuffer = unsafe {
                            MemoryObject::from_handle(
                                handoff_info.get_as_memory_object("framebuffer").unwrap(),
                                width * height * 4,
                                MemoryObjectFlags::WRITABLE,
                            )
                        };
                        let channel: Channel<(), ()> =
                            Channel::new_from_handle(handoff_info.get_as_channel("channel").unwrap());

                        // Map the framebuffer into our address space
                        const FRAMEBUFFER_ADDDRESS: usize = 0x00000005_00000000;
                        let framebuffer = unsafe { framebuffer.map_at(FRAMEBUFFER_ADDDRESS).unwrap() };

                        spawn_framebuffer(
                            framebuffer,
                            channel,
                            width,
                            height,
                            input_receiver.take().unwrap(),
                            &service_host_client,
                        );
                    } else if device_info.get_as_str("hid.type").is_some() {
                        info!("Found HID-compatible input device: {}", name);

                        let channel: Channel<(), PlatformBusInputEvent> =
                            Channel::new_from_handle(handoff_info.get_as_channel("hid.channel").unwrap());
                        let input_sender = input_sender.clone();

                        std::poplar::rt::spawn(async move {
                            loop {
                                let event = channel.receive().await.unwrap();
                                match event {
                                    PlatformBusInputEvent::KeyPressed { key, state } => match key {
                                        Key::BtnLeft => {
                                            info!("Left mouse button");
                                        }
                                        Key::BtnRight => {
                                            info!("Right mouse button");
                                        }
                                        Key::BtnMiddle => {
                                            info!("Middle mouse button");
                                        }
                                        Key::BtnSide | Key::BtnExtra => {}

                                        other => {
                                            input_sender
                                                .send(InputEvent::KeyPressed(map_key(key, state).unwrap()))
                                                .await
                                                .unwrap();
                                        }
                                    },
                                    PlatformBusInputEvent::RelX(value) => {
                                        input_sender.send(InputEvent::RelX(value)).await.unwrap();
                                    }
                                    PlatformBusInputEvent::RelY(value) => {
                                        input_sender.send(InputEvent::RelY(value)).await.unwrap();
                                    }
                                    PlatformBusInputEvent::RelWheel(_) => {}
                                    _ => (),
                                }
                            }
                        });
                    } else {
                        panic!("Passed unsupported device!");
                    }
                }
            }
        }
    });

    std::poplar::rt::enter_loop();
}

// TODO: we should probably be able to define a keymap in a more data-oriented way in the future
// TODO: I'm not sure if we'll want to map everything to UTF-8 or if some would need different
// control-esque types or something?
pub fn map_key(usage: Key, state: KeyState) -> Option<char> {
    match (usage, state.shift()) {
        (Key::KeyA, false) => Some('a'),
        (Key::KeyA, true) => Some('A'),
        (Key::KeyB, false) => Some('b'),
        (Key::KeyB, true) => Some('B'),
        (Key::KeyC, false) => Some('c'),
        (Key::KeyC, true) => Some('C'),
        (Key::KeyD, false) => Some('d'),
        (Key::KeyD, true) => Some('D'),
        (Key::KeyE, false) => Some('e'),
        (Key::KeyE, true) => Some('E'),
        (Key::KeyF, false) => Some('f'),
        (Key::KeyF, true) => Some('F'),
        (Key::KeyG, false) => Some('g'),
        (Key::KeyG, true) => Some('G'),
        (Key::KeyH, false) => Some('h'),
        (Key::KeyH, true) => Some('H'),
        (Key::KeyI, false) => Some('i'),
        (Key::KeyI, true) => Some('I'),
        (Key::KeyJ, false) => Some('j'),
        (Key::KeyJ, true) => Some('J'),
        (Key::KeyK, false) => Some('k'),
        (Key::KeyK, true) => Some('K'),
        (Key::KeyL, false) => Some('l'),
        (Key::KeyL, true) => Some('L'),
        (Key::KeyM, false) => Some('m'),
        (Key::KeyM, true) => Some('M'),
        (Key::KeyN, false) => Some('n'),
        (Key::KeyN, true) => Some('N'),
        (Key::KeyO, false) => Some('o'),
        (Key::KeyO, true) => Some('O'),
        (Key::KeyP, false) => Some('p'),
        (Key::KeyP, true) => Some('P'),
        (Key::KeyQ, false) => Some('q'),
        (Key::KeyQ, true) => Some('Q'),
        (Key::KeyR, false) => Some('r'),
        (Key::KeyR, true) => Some('R'),
        (Key::KeyS, false) => Some('s'),
        (Key::KeyS, true) => Some('S'),
        (Key::KeyT, false) => Some('t'),
        (Key::KeyT, true) => Some('T'),
        (Key::KeyU, false) => Some('u'),
        (Key::KeyU, true) => Some('U'),
        (Key::KeyV, false) => Some('v'),
        (Key::KeyV, true) => Some('V'),
        (Key::KeyW, false) => Some('w'),
        (Key::KeyW, true) => Some('W'),
        (Key::KeyX, false) => Some('x'),
        (Key::KeyX, true) => Some('X'),
        (Key::KeyY, false) => Some('y'),
        (Key::KeyY, true) => Some('Y'),
        (Key::KeyZ, false) => Some('z'),
        (Key::Key1, false) => Some('1'),
        (Key::Key1, true) => Some('!'),
        (Key::Key2, false) => Some('2'),
        (Key::Key2, true) => Some('@'),
        (Key::Key3, false) => Some('3'),
        (Key::Key3, true) => Some('#'),
        (Key::Key4, false) => Some('4'),
        (Key::Key4, true) => Some('$'),
        (Key::Key5, false) => Some('5'),
        (Key::Key5, true) => Some('%'),
        (Key::Key6, false) => Some('6'),
        (Key::Key6, true) => Some('^'),
        (Key::Key7, false) => Some('7'),
        (Key::Key7, true) => Some('&'),
        (Key::Key8, false) => Some('8'),
        (Key::Key8, true) => Some('*'),
        (Key::Key9, false) => Some('9'),
        (Key::Key9, true) => Some('('),
        (Key::Key0, false) => Some('0'),
        (Key::Key0, true) => Some(')'),
        (Key::KeyReturn, _) => Some('\n'),
        (Key::KeyEscape, _) => None,
        /*
         * XXX: confusingly, `KeyDelete` is actually backspace, and delete is `KeyDeleteForward`.
         * We map to an `0x7f` ASCII `DEL`, which differs from an ASCII backspace (`0x08`), which
         * moves the cursor but does not delete a character.
         */
        (Key::KeyDelete, _) => Some('\x7f'),
        (Key::KeyTab, _) => Some('\t'),
        (Key::KeySpace, _) => Some(' '),
        (Key::KeyDash, false) => Some('-'),
        (Key::KeyDash, true) => Some('_'),
        (Key::KeyEquals, false) => Some('='),
        (Key::KeyEquals, true) => Some('+'),
        (Key::KeyLeftBracket, false) => Some('['),
        (Key::KeyLeftBracket, true) => Some('{'),
        (Key::KeyRightBracket, false) => Some(']'),
        (Key::KeyRightBracket, true) => Some('}'),
        (Key::KeyForwardSlash, false) => Some('\\'),
        (Key::KeyForwardSlash, true) => Some('|'),
        (Key::KeyPound, _) => Some('#'),
        (Key::KeySemicolon, false) => Some(';'),
        (Key::KeySemicolon, true) => Some(':'),
        (Key::KeyApostrophe, false) => Some('\''),
        (Key::KeyApostrophe, true) => Some('"'),
        (Key::KeyGrave, false) => Some('`'),
        (Key::KeyGrave, true) => Some('~'),
        (Key::KeyComma, false) => Some(','),
        (Key::KeyComma, true) => Some('<'),
        (Key::KeyDot, false) => Some('.'),
        (Key::KeyDot, true) => Some('>'),
        (Key::KeyBackSlash, false) => Some('/'),
        (Key::KeyBackSlash, true) => Some('?'),
        _ => None,
    }
}
