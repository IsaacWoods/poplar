//! `bitfield` is a library for defining structured bitfields in Rust. It is heavily inspired by the
//! [`mycelium_bitfield`](https://github.com/hawkw/mycelium/tree/main/bitfield) library, and started
//! as an attempt to make that library allow contructing bitfields in constant-evaluation contexts.
//! This library aims to allow `const`-compatible bitfield construction via procedural macros, with
//! very similar syntax and overall functionality to `mycelium_bitfield`.

#![no_std]
#![feature(const_trait_impl, never_type, const_ops, const_cmp)]

pub mod from_bits;
pub mod pack;

pub use bitfield_macro::bitfield;
pub use from_bits::FromBits;
pub use pack::Packer;
