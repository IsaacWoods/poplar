#![no_std]
#![feature(decl_macro, naked_functions, type_ascription, const_fn_trait_bound)]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(test)]
#[macro_use]
extern crate std;

pub mod hw;
pub mod kernel_map;
pub mod paging;
