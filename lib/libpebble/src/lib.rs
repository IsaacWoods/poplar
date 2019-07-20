#![no_std]
#![feature(asm)]

pub mod caps;
pub mod object;
pub mod syscall;

pub use object::KernelObjectId;
