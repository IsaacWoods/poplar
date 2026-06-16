//! BnB (bits n' bobs) is a library for small utilities that can be used from the kernel and
//! userspace.

#![no_std]
#![feature(const_trait_impl)]

pub mod bit_ops;

pub use bit_ops::BitOps;
