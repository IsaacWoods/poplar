//! A `no_std` TOML deserializer build for embedded systems. Can be used without an allocator.

#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

mod de;
mod error;
mod lexer;
mod peeking;

pub use de::{from_str, Deserializer};
pub use error::{Error, ErrorKind, Expected};
