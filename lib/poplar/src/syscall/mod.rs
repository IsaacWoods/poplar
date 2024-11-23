pub mod get_framebuffer;
pub mod pci;
pub mod result;

use core::mem::MaybeUninit;

pub use get_framebuffer::{get_framebuffer, FramebufferInfo, GetFramebufferError, PixelFormat};
pub use pci::{pci_get_info, PciGetInfoError};

cfg_if::cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        pub mod raw_x86_64;
        pub use raw_x86_64 as raw;
    } else if #[cfg(target_arch = "riscv64")] {
        pub mod raw_riscv;
        pub use raw_riscv as raw;
    } else {
        compile_error!("Poplar does not support this target architecture!");
    }
}

use crate::Handle;
use bit_field::BitField;
use result::{define_error_type, handle_from_syscall_repr, status_from_syscall_repr};

pub const SYSCALL_YIELD: usize = 0;
pub const SYSCALL_EARLY_LOG: usize = 1;
pub const SYSCALL_GET_FRAMEBUFFER: usize = 2;
pub const SYSCALL_CREATE_MEMORY_OBJECT: usize = 3;
pub const SYSCALL_MAP_MEMORY_OBJECT: usize = 4;
pub const SYSCALL_CREATE_CHANNEL: usize = 5;
pub const SYSCALL_SEND_MESSAGE: usize = 6;
pub const SYSCALL_GET_MESSAGE: usize = 7;
pub const SYSCALL_WAIT_FOR_MESSAGE: usize = 8;
pub const SYSCALL_PCI_GET_INFO: usize = 11;
pub const SYSCALL_WAIT_FOR_EVENT: usize = 12;
pub const SYSCALL_POLL_INTEREST: usize = 13;
pub const SYSCALL_CREATE_ADDRESS_SPACE: usize = 14;
pub const SYSCALL_SPAWN_TASK: usize = 15;

pub fn yield_to_kernel() {
    unsafe {
        raw::syscall0(SYSCALL_YIELD);
    }
}

define_error_type!(EarlyLogError {
    MessageTooLong => 1,
    MessageNotValidUtf8 => 2,
    TaskDoesNotHaveCorrectCapability => 3,
});

pub fn early_log(message: &str) -> Result<(), EarlyLogError> {
    status_from_syscall_repr(unsafe {
        raw::syscall2(SYSCALL_EARLY_LOG, message.len(), message as *const str as *const u8 as usize)
    })
}

define_error_type!(CreateMemoryObjectError {
    InvalidFlags => 1,
    InvalidSize => 2,
    InvalidPhysicalAddressPointer => 3,
});

bitflags::bitflags! {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub struct MemoryObjectFlags: u32 {
        const WRITABLE = 1 << 0;
        const EXECUTABLE = 1 << 1;
    }
}

/// Create a MemoryObject kernel object of the given size (in bytes). Returns a handle to the new
/// MemoryObject, if the call was successful.
pub unsafe fn create_memory_object(
    size: usize,
    flags: MemoryObjectFlags,
    physical_address_ptr: *mut usize,
) -> Result<Handle, CreateMemoryObjectError> {
    handle_from_syscall_repr(unsafe {
        raw::syscall3(SYSCALL_CREATE_MEMORY_OBJECT, size, flags.bits() as usize, physical_address_ptr as usize)
    })
}

define_error_type!(MapMemoryObjectError {
    InvalidMemoryObjectHandle => 1,
    InvalidAddressSpaceHandle => 2,
    RegionAlreadyMapped => 3,
    AddressPointerInvalid => 4,
});

pub unsafe fn map_memory_object(
    memory_object: Handle,
    address_space: Handle,
    virtual_address: Option<usize>,
    address_pointer: *mut usize,
) -> Result<(), MapMemoryObjectError> {
    status_from_syscall_repr(unsafe {
        raw::syscall4(
            SYSCALL_MAP_MEMORY_OBJECT,
            memory_object.0 as usize,
            address_space.0 as usize,
            if virtual_address.is_some() { virtual_address.unwrap() } else { 0x0 },
            address_pointer as usize,
        )
    })
}

define_error_type!(CreateChannelError {
    InvalidHandleAddress => 1,
});

pub fn create_channel() -> Result<(Handle, Handle), CreateChannelError> {
    let mut other_end: MaybeUninit<Handle> = MaybeUninit::uninit();
    let one_end = handle_from_syscall_repr(unsafe {
        raw::syscall1(SYSCALL_CREATE_CHANNEL, other_end.as_mut_ptr() as usize)
    })?;
    Ok((one_end, unsafe { other_end.assume_init() }))
}

pub const CHANNEL_MAX_NUM_BYTES: usize = 4096;
pub const CHANNEL_MAX_NUM_HANDLES: usize = 4;

define_error_type!(SendMessageError {
    /// The `Channel` handle is invalid.
    InvalidChannelHandle => 1,
    /// The `Channel` handle isn't a `Channel`.
    NotAChannel => 2,
    /// The `Channel` handle must have the `SEND` right to use the `send_message` system call.
    ChannelCannotSend => 3,
    /// A handle to be transferred is invalid.
    InvalidTransferredHandle => 4,
    /// Transferred handles must have the `TRANSFER` right.
    CannotTransferHandle => 5,
    BytesAddressInvalid => 6,
    TooManyBytes => 7,
    HandlesAddressInvalid => 8,
    TooManyHandles => 9,
    OtherEndDisconnected => 10,
});

pub fn send_message(channel: Handle, bytes: &[u8], handles: &[Handle]) -> Result<(), SendMessageError> {
    status_from_syscall_repr(unsafe {
        raw::syscall5(
            SYSCALL_SEND_MESSAGE,
            channel.0 as usize,
            if bytes.len() == 0 { 0x0 } else { bytes.as_ptr() as usize },
            bytes.len(),
            if handles.len() == 0 { 0x0 } else { handles.as_ptr() as usize },
            handles.len(),
        )
    })
}

define_error_type!(GetMessageError {
    InvalidChannelHandle => 1,
    NotAChannel => 2,
    NoMessage => 3,
    BytesAddressInvalid => 4,
    BytesBufferTooSmall => 5,
    HandlesAddressInvalid => 6,
    HandlesBufferTooSmall => 7,
});

pub fn get_message<'b, 'h>(
    channel: Handle,
    byte_buffer: &'b mut [u8],
    handle_buffer: &'h mut [Handle],
) -> Result<(&'b mut [u8], &'h mut [Handle]), GetMessageError> {
    let result = unsafe {
        raw::syscall5(
            SYSCALL_GET_MESSAGE,
            channel.0 as usize,
            if byte_buffer.len() == 0 { 0x0 } else { byte_buffer.as_ptr() as usize },
            byte_buffer.len(),
            if handle_buffer.len() == 0 { 0x0 } else { handle_buffer.as_ptr() as usize },
            handle_buffer.len(),
        )
    };
    status_from_syscall_repr(result.get_bits(0..16))?;

    let valid_bytes_len = result.get_bits(16..32);
    let valid_handles_len = result.get_bits(32..48);

    Ok((&mut byte_buffer[0..valid_bytes_len], &mut handle_buffer[0..valid_handles_len]))
}

define_error_type!(WaitForEventError {
    InvalidHandle => 1,
    NotAnEvent => 2,
    /// No event has occured, and the caller does not want the kernel to block.
    NoEvent => 3,
});

pub fn wait_for_event(event: Handle, block: bool) -> Result<(), WaitForEventError> {
    let result = unsafe { raw::syscall2(SYSCALL_WAIT_FOR_EVENT, event.0 as usize, if block { 1 } else { 0 }) };
    status_from_syscall_repr(result)
}

define_error_type!(PollInterestError {
    InvalidHandle => 1,
});

pub fn poll_interest(object: Handle) -> Result<bool, PollInterestError> {
    let result = unsafe { raw::syscall1(SYSCALL_POLL_INTEREST, object.0 as usize) };
    status_from_syscall_repr(result.get_bits(0..16))?;
    Ok(result.get_bits(16..64) != 0)
}

define_error_type!(CreateAddressSpaceError {});

pub fn create_address_space() -> Result<Handle, CreateAddressSpaceError> {
    handle_from_syscall_repr(unsafe { raw::syscall0(SYSCALL_CREATE_ADDRESS_SPACE) })
}

define_error_type!(SpawnTaskError {
    InvalidTaskName => 1,
    NotAnAddressSpace => 2,
    InvalidHandleToTransfer => 3,
});

#[repr(C)]
pub struct SpawnTaskDetails {
    pub name_ptr: *const u8,
    pub name_len: usize,
    pub entry_point: usize,
    pub address_space: u32,
    pub object_array: *const u32,
    pub object_array_len: usize,
}

pub fn spawn_task(
    task_name: &str,
    address_space: Handle,
    entry_point: usize,
    objects: &[Handle],
) -> Result<Handle, SpawnTaskError> {
    let details = SpawnTaskDetails {
        name_ptr: task_name as *const str as *const u8,
        name_len: task_name.len(),
        entry_point,
        address_space: address_space.0,
        object_array: objects as *const [Handle] as *const u32,
        object_array_len: objects.len(),
    };

    handle_from_syscall_repr(unsafe {
        raw::syscall1(SYSCALL_SPAWN_TASK, &details as *const SpawnTaskDetails as usize)
    })
}
