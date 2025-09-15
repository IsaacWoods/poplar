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

    // Stuff for providing built-in functions
    platform_bus_inspect: Channel<(), platform_bus::PlatformBusInspect>,
}

impl Console {
    pub fn new(service_host_client: &ServiceHostClient, writer: ConsoleWriter) -> Console {
        let mut vm = Vm::new();
        let platform_bus_inspect = service_host_client.subscribe_service("platform_bus.inspect").unwrap();

        let mut print_writer = writer.clone();
        vm.define_native_fn("print", move |args| {
            let mut print_writer = print_writer.clone();
            assert!(args.len() == 1);
            let value = args.get(0).unwrap();
            writeln!(&mut print_writer, "PRINT: {:?}", value).unwrap();
            Value::Unit
        });

        Console { vm, writer, platform_bus_inspect }
        let prelude = Parser::new(GINKGO_PRELUDE).parse().expect("Parse error in prelude");
        vm.interpret(prelude).expect("Runtime error in prelude");

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
