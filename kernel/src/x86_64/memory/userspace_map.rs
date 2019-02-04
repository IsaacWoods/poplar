use x86_64::memory::{paging::PAGE_SIZE, VirtualAddress};

pub const KERNEL_SPACE_START: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0xffffffff_80000000) };
pub const KERNEL_SPACE_END: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0xffffffff_ffffffff) };

pub const INITIAL_STACK_SIZE: usize = PAGE_SIZE;

pub const MEMORY_OBJECTS_START: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0x00000006_00000000) };
pub const RECEIVE_BUFFERS_START: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0x00000005_00000000) };
pub const SEND_BUFFERS_START: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0x00000004_00000000) };
pub const KERNEL_STACKS_START: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0x00000003_00000000) };
pub const USER_STACKS_START: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0x00000002_00000000) };
pub const HEAP_START: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0x00000001_00000000) };

pub const IMAGE_START: VirtualAddress =
    unsafe { VirtualAddress::new_unchecked(0x00000000_00010000) };
