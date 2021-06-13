#![no_std]
#![feature(asm, decl_macro, global_asm, naked_functions, type_ascription, const_fn_trait_bound)]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(test)]
#[macro_use]
extern crate std;

pub mod hw;
pub mod kernel_map;
pub mod paging;
