//! A no_std toml deserializer for embedded systems
//! Full toml support for deserializing into structs that implement `serde::Deserialize`.
//!
//! Requires no memory allocations and is likely much faster than toml-rs.
//! Supports deserialization only

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
