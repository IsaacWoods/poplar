#![no_std]
#![feature(llvm_asm, decl_macro, const_generics)]

pub mod caps;
pub mod syscall;

#[cfg(feature = "can_alloc")]
pub mod early_logger;

#[cfg(feature = "can_alloc")]
extern crate alloc;

use core::{convert::TryFrom, num::TryFromIntError};

/// A `Handle` is used to represent a task's access to a kernel object. It is allocated by the kernel and is unique
/// to the task to which it is issued - a kernel object can have handles in multiple tasks (and the numbers will
/// not be shared between those tasks).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Handle(pub u32);

pub const ZERO_HANDLE: Handle = Handle(0);

/*
 * Often, handles are passed in single syscall parameters, and need to be turned into `Handle`s fallibly.
 * XXX: this cannot be used to convert `Result` types that contain handles - it simply does the bounds check!
 */
impl TryFrom<usize> for Handle {
    type Error = TryFromIntError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Ok(Handle(u32::try_from(value)?))
    }
}

bitflags::bitflags! {
    struct HandleRights: u32 {
        /// Whether the handle's owner can use it to modify the kernel object it points to. What is means to
        /// "modify" a kernel object differs depending on the type of the kernel object.
        const MODIFY = 0b1;
        /// Whether the handle can be duplicated.
        const DUPLICATE = 0b10;
        /// Whether the handle can be transferred over a `Channel`.
        const TRANSFER = 0b100;
        /// For `MemoryObject`s, whether the memory can be mapped into the handle owner's `AddressSpace`.
        const MAP = 0x1000;
        /// For `Channel` ends, whether the `send_message` system call can be used on this `Channel` end.
        const SEND = 0x1_0000;
        /// For `Channel` ends, whether the `receive_message` & co. system calls can be used on this `Channel` end.
        const RECEIVE = 0x10_0000;
    }
}
