use x86_64::memory::{FrameSize, Size4KiB, VirtualAddress, MEBIBYTES_TO_BYTES};

pub const KERNEL_SPACE_START: VirtualAddress = unsafe { VirtualAddress::new_unchecked(0xffffffff_80000000) };
pub const KERNEL_SPACE_END: VirtualAddress = unsafe { VirtualAddress::new_unchecked(0xffffffff_ffffffff) };

/// The initial size of a task's user and kernel stacks. Must be a multiple of the page size.
pub const INITIAL_STACK_SIZE: usize = 4 * Size4KiB::SIZE;
/// The maximum size of each task's usermode stack. This is used to space each task's stack slots
/// out in the stack window.
pub const MAX_STACK_SIZE: usize = 2 * MEBIBYTES_TO_BYTES;

pub const MEMORY_OBJECTS_START: VirtualAddress = unsafe { VirtualAddress::new_unchecked(0x00000006_00000000) };
pub const RECEIVE_BUFFERS_START: VirtualAddress = unsafe { VirtualAddress::new_unchecked(0x00000005_00000000) };
pub const SEND_BUFFERS_START: VirtualAddress = unsafe { VirtualAddress::new_unchecked(0x00000004_00000000) };
pub const USER_STACKS_START: VirtualAddress = unsafe { VirtualAddress::new_unchecked(0x00000002_00000000) };
pub const HEAP_START: VirtualAddress = unsafe { VirtualAddress::new_unchecked(0x00000001_00000000) };

pub const IMAGE_START: VirtualAddress = unsafe { VirtualAddress::new_unchecked(0x00000000_00010000) };
