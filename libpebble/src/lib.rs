#![no_std]
#![feature(asm)]

pub mod syscall;

pub type Index = u16;
pub type Generation = u16;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ProcessId {
    pub index: Index,
    pub generation: Generation,
}
