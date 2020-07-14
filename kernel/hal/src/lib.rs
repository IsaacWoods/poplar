#![no_std]
#![feature(decl_macro, step_trait, step_trait_ext)]

#[cfg(test)]
#[macro_use]
extern crate std;

pub mod boot_info;
pub mod memory;
