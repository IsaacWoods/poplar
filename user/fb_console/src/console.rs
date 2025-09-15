use crate::ConsoleWriter;
use ginkgo::{
    parse::Parser,
    vm::{Value, Vm},
};
use service_host::ServiceHostClient;
use std::{fmt::Write, poplar::channel::Channel};

const GINKGO_PRELUDE: &'static str = include_str!("prelude.ginkgo");

/// `Console` implements the logic of the console itself - it is fed input, maintains the Ginkgo
/// interpreter and state needed to interface with the rest of the system, and returns output.
pub struct Console {
    vm: Vm,
    writer: ConsoleWriter,
}

impl Console {
    pub fn new(service_host_client: &ServiceHostClient, writer: ConsoleWriter) -> Console {
        let mut vm = Vm::new();

        let prelude = Parser::new(GINKGO_PRELUDE).parse().expect("Parse error in prelude");
        vm.interpret(prelude).expect("Runtime error in prelude");

        {
            let writer = writer.clone();
            vm.define_native_fn("print", move |args| {
                let mut writer = writer.clone();

                assert!(args.len() == 1);
                let value = args.get(0).unwrap();

                writeln!(&mut writer, "PRINT: {:?}", value).unwrap();
                Value::Unit
            });
        }

        {
            let platform_bus_inspect: Channel<(), platform_bus::PlatformBusInspect> =
                service_host_client.subscribe_service("platform_bus.inspect").unwrap();
            let writer = writer.clone();
            vm.define_native_fn("inspect_pbus", move |args| {
                let mut writer = writer.clone();
                assert!(args.len() == 0);

                // TODO: think about how the Ginkgo VM should interact with async stuff. Blocking
                // til the pbus replies to us is not fantastic. We should probably have some sort
                // of worker system to delegate these things to / utilise the userspace runtime?
                platform_bus_inspect.send(&()).unwrap();
                let reply = platform_bus_inspect.receive_blocking().unwrap();

                for device in &reply.devices {
                    writeln!(&mut writer, "Device: {}", device.name).unwrap();
                    for (property, value) in &device.properties {
                        writeln!(&mut writer, "    {}: {:?}", property, value);
                    }
                }

                Value::Unit
            });
        }

        Console { vm, writer }
    }

    pub fn interpret(&mut self, s: &str) {
        let parser = Parser::new(s);
        match parser.parse() {
            Ok(chunk) => match self.vm.interpret(chunk) {
                Ok(_) => {
                    if let Some(result) = self.vm.stack.pop() {
                        writeln!(&mut self.writer, "Result: {:?}", result).unwrap();
                    }
                }
                Err(err) => {
                    writeln!(&mut self.writer, "Runtime error: {}", err).unwrap();
                }
            },
            Err(err) => {
                writeln!(&mut self.writer, "Parse error: {}", err).unwrap();
            }
        }
    }
}
