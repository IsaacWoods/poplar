#![no_std]

extern crate alloc;

// mod de;
// mod ser;

// pub use de::Deserializer;
// pub use ser::Serializer;

use alloc::string::{String, ToString};
use core::fmt;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Error {
    Custom(String),
}

impl serde::ser::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Error::Custom(msg.to_string())
    }
}

impl serde::de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: fmt::Display,
    {
        Error::Custom(msg.to_string())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        unimplemented!()
    }
}

type Result<T> = core::result::Result<T, Error>;

// XXX: in the future, we'll be able to implement Writer for a "slice" of a message buffer shared between a task
// and the kernel
trait Writer {
    fn write(&mut self, buf: &[u8]) -> Result<()>;
}
