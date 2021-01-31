#![no_std]
#![feature(asm, decl_macro, const_fn, global_asm, naked_functions, type_ascription, unsafe_block_in_unsafe_fn)]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod hw;
pub mod kernel_map;
pub mod paging;
