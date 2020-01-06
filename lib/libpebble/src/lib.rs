#![no_std]
#![feature(asm, decl_macro)]

pub mod caps;
pub mod object;
pub mod syscall;

pub use object::KernelObjectId;
