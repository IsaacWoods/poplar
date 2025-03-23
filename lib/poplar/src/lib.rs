#![no_std]
#![feature(decl_macro, never_type, allocator_api, ptr_as_uninit)]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(feature = "can_alloc")]
extern crate alloc;

#[cfg(feature = "can_alloc")]
pub mod channel;
#[cfg(feature = "ddk")]
pub mod ddk;
#[cfg(feature = "can_alloc")]
pub mod early_logger;
pub mod event;
pub mod interrupt;
pub mod manifest;
pub mod memory_object;
#[cfg(feature = "async")]
pub mod rt;
pub mod syscall;

use core::num::TryFromIntError;

/// A `Handle` is used to represent a task's access to a kernel object. It is allocated by the kernel and is unique
/// to the task to which it is issued - a kernel object can have handles in multiple tasks (and the numbers will
/// not be shared between those tasks).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Handle(pub u32);

impl Handle {
    pub const ZERO: Handle = Handle(0);
}

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

#[cfg(feature = "ptah")]
impl ptah::Serialize for Handle {
    fn serialize<W>(&self, serializer: &mut ptah::Serializer<W>) -> ptah::ser::Result<()>
    where
        W: ptah::Writer,
    {
        serializer.serialize_handle(self.0 as ptah::Handle)
    }
}

#[cfg(feature = "ptah")]
impl<'de> ptah::Deserialize<'de> for Handle {
    fn deserialize(deserializer: &mut ptah::Deserializer<'de>) -> ptah::de::Result<Handle> {
        Ok(Handle(deserializer.deserialize_handle()?))
    }
}

// TODO: I don't think rights are implemented at all are they? Work out if we want them / remove
// this.
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
