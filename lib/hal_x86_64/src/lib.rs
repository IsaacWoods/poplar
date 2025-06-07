#![no_std]
#![feature(decl_macro, type_ascription, if_let_guard)]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(test)]
#[macro_use]
extern crate std;

pub mod hw;
pub mod kernel_map;
pub mod paging;

#[inline(always)]
pub fn breakpoint() {
    unsafe {
        core::arch::asm!("int3");
    }
}
