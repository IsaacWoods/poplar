//! `fb_console` is a console running on top of a framebuffer device, either provided through the
//! kernel or by a driver for a graphics-capable device.

use gfxconsole::{Format, Framebuffer, GfxConsole, Pixel, Rgb32};
use ginkgo::{
    ast::BindingResolver,
    interpreter::{Interpreter, Value},
    parse::Parser,
};
use log::info;
use platform_bus::{
    input::InputEvent as PlatformBusInputEvent,
    DeviceDriverMessage,
    DeviceDriverRequest,
    Filter,
    Property,
};
use spinning_top::Spinlock;
use std::{
    fmt::Write,
    poplar::{
        caps::{CapabilitiesRepr, CAP_EARLY_LOGGING, CAP_GET_FRAMEBUFFER, CAP_PADDING, CAP_SERVICE_USER},
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
}

struct Console {
    framebuffer: MappedMemoryObject,
    control_channel: Channel<(), ()>,
    width: usize,
    height: usize,
    console: Spinlock<GfxConsole<Rgb32>>,
    input_events: thingbuf::mpsc::Receiver<InputEvent>,
}

fn spawn_framebuffer(
    framebuffer: MappedMemoryObject,
    channel: Channel<(), ()>,
    width: usize,
    height: usize,
    input_events: thingbuf::mpsc::Receiver<InputEvent>,
) {
    let console = Spinlock::new(GfxConsole::new(
        Framebuffer { ptr: framebuffer.ptr() as *mut Pixel<Rgb32>, width, height, stride: width },
        Rgb32::pixel(0x00, 0x00, 0x00, 0x00),
        Rgb32::pixel(0xff, 0xff, 0xff, 0xff),
    ));
    let console = Console { framebuffer, control_channel: channel, width, height, console, input_events };

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
                                    if let Some(value) = interpreter.eval_stmt(statement) {
                                        result = Some(value);
                                    }
                                }

                                write!(console.console.lock(), "{}", key);
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

                            other => {
                                write!(console.console.lock(), "{}", key).unwrap();
                                current_line.push(key);
                                needs_redraw = true;
                            }
                        }
                    }
                    InputEvent::Default => panic!(),
                }
            }

            if needs_redraw {
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

        // We act as a device driver to find framebuffers and input devices
        let platform_bus_device_channel: Channel<DeviceDriverMessage, DeviceDriverRequest> =
            Channel::subscribe_to_service("platform_bus.device_driver").unwrap();
        platform_bus_device_channel
            .send(&DeviceDriverMessage::RegisterInterest(vec![
                Filter::Matches(String::from("type"), Property::String("framebuffer".to_string())),
                Filter::Matches(String::from("hid.type"), Property::String("keyboard".to_string())),
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

                        spawn_framebuffer(framebuffer, channel, width, height, input_receiver.take().unwrap());
                    } else if let Some("keyboard") = device_info.get_as_str("hid.type") {
                        info!("Found HID-compatible keyboard: {}", name);

                        let channel: Channel<(), PlatformBusInputEvent> =
                            Channel::new_from_handle(handoff_info.get_as_channel("hid.channel").unwrap());
                        let input_sender = input_sender.clone();

                        std::poplar::rt::spawn(async move {
                            loop {
                                let event = channel.receive().await.unwrap();
                                match event {
                                    PlatformBusInputEvent::KeyPressed { key, .. } => {
                                        input_sender.send(InputEvent::KeyPressed(key)).await.unwrap();
                                    }
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

#[used]
#[link_section = ".caps"]
pub static mut CAPS: CapabilitiesRepr<4> =
    CapabilitiesRepr::new([CAP_EARLY_LOGGING, CAP_GET_FRAMEBUFFER, CAP_SERVICE_USER, CAP_PADDING]);
