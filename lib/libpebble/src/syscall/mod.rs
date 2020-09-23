pub mod get_framebuffer;
pub mod pci;
pub mod result;

pub use get_framebuffer::{get_framebuffer, FramebufferInfo, GetFramebufferError, PixelFormat};

cfg_if::cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        pub mod raw_x86_64;
        pub use raw_x86_64 as raw;
    } else {
        compile_error!("libpebble does not support this target architecture!");
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
pub const SYSCALL_REGISTER_SERVICE: usize = 9;
pub const SYSCALL_SUBSCRIBE_TO_SERVICE: usize = 10;
pub const SYSCALL_PCI_GET_INFO: usize = 11;

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
    InvalidVirtualAddress => 1,
    InvalidFlags => 2,
    InvalidSize => 3,
});

/// Create a MemoryObject kernel object at the given virtual address, with the given size (in bytes). Returns a
/// handle to the new MemoryObject, if the call was successful.
pub fn create_memory_object(
    virtual_address: usize,
    size: usize,
    writable: bool,
    executable: bool,
) -> Result<Handle, CreateMemoryObjectError> {
    let mut flags = 0usize;
    flags.set_bit(0, writable);
    flags.set_bit(1, executable);

    handle_from_syscall_repr(unsafe { raw::syscall3(SYSCALL_CREATE_MEMORY_OBJECT, virtual_address, size, flags) })
}

define_error_type!(MapMemoryObjectError {
    InvalidHandle => 1,
    RegionAlreadyMapped => 2,
    NotAMemoryObject => 3,
    NotAnAddressSpace => 4,
    AddressPointerInvalid => 5,
});

pub unsafe fn map_memory_object(
    memory_object: Handle,
    address_space: Handle,
    address_pointer: *mut usize,
) -> Result<(), MapMemoryObjectError> {
    status_from_syscall_repr(unsafe {
        raw::syscall3(
            SYSCALL_MAP_MEMORY_OBJECT,
            memory_object.0 as usize,
            address_space.0 as usize,
            address_pointer as usize,
        )
    })
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

pub const SERVICE_NAME_MAX_LENGTH: usize = 256;

define_error_type!(RegisterServiceError {
    TaskDoesNotHaveCorrectCapability => 1,
    NamePointerNotValid => 2,
    /// Name must be greater than `0` bytes, and not greater than `256` bytes.
    NameLengthNotValid => 3,
});

pub fn register_service(name: &str) -> Result<Handle, RegisterServiceError> {
    handle_from_syscall_repr(unsafe {
        raw::syscall2(SYSCALL_REGISTER_SERVICE, name.len(), name.as_ptr() as usize)
    })
}

define_error_type!(SubscribeToServiceError {
    TaskDoesNotHaveCorrectCapability => 1,
    NamePointerNotValid => 2,
    /// Name must be greater than `0` bytes, and not greater than `256` bytes.
    NameLengthNotValid => 3,
    NoServiceWithThatName => 4,
});

pub fn subscribe_to_service(name: &str) -> Result<Handle, SubscribeToServiceError> {
    handle_from_syscall_repr(unsafe {
        raw::syscall2(SYSCALL_SUBSCRIBE_TO_SERVICE, name.len(), name.as_ptr() as usize)
    })
}
