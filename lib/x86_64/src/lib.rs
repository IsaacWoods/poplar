#![no_std]
#![feature(asm, decl_macro, never_type, step_trait, const_fn, type_ascription, box_syntax, arbitrary_self_types)]
#![allow(unknown_lints)]

#[cfg(test)]
#[macro_use]
extern crate std;

#[cfg(feature = "kernel")]
extern crate alloc;

pub mod boot;
pub mod hw;
pub mod memory;
