#![no_std]
#![feature(asm, decl_macro, const_generics)]

pub mod caps;
pub mod syscall;

/// A `Handle` is used to represent a task's access to a kernel object. It is allocated by the kernel and is unique
/// to the task to which it is issued - a kernel object can have handles in multiple tasks (and the numbers will
/// not be shared between those tasks).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Handle(pub(self) u16);
