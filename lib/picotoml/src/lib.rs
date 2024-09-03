//! A `no_std` TOML deserializer build for embedded systems. Can be used without an allocator.

#![no_std]

extern crate alloc;

#[cfg(test)]
extern crate std;

mod de;
mod error;
mod lexer;
mod peeking;

pub use de::{from_str, Deserializer};
pub use error::{Error, ErrorKind, Expected};
