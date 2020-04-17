use alloc::string::String;
use core::fmt::Write;
use log::{Log, Metadata, Record};

pub struct EarlyLogger;

impl Log for EarlyLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let mut s = String::new();
            write!(s, "{}", record.args()).unwrap();
            crate::syscall::early_log(&s).unwrap();
        }
    }

    fn flush(&self) {}
}
