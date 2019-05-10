mod events;
mod memory;
mod pool_ptr;
mod protocols;

pub use self::{events::*, memory::*, pool_ptr::*, protocols::*};
use crate::{
    memory::{MemoryDescriptor, MemoryType},
    uefi::{Char16, Guid, Handle, Status, TableHeader},
};
use core::{
    char::{decode_utf16, REPLACEMENT_CHARACTER},
    fmt,
    slice,
    str::from_utf8_unchecked_mut,
    sync::atomic::AtomicPtr,
};
use x86_64::memory::PhysicalAddress;

#[repr(C)]
pub struct BootServices {
    pub hdr: TableHeader,

    // Task Priority Services
    pub _raise_tpl: extern "win64" fn(new_tpl: TaskPriorityLevel) -> Status,
    pub _restore_tpl: extern "win64" fn(old_tpl: TaskPriorityLevel) -> Status,

    // Memory Services
    pub _allocate_pages: extern "win64" fn(
        allocation_type: AllocateType,
        memory_type: MemoryType,
        pages: usize,
        memory: &mut PhysicalAddress,
    ) -> Status,
    pub _free_pages: extern "win64" fn(memory: PhysicalAddress, pages: usize) -> Status,
    pub _get_memory_map: extern "win64" fn(
        memory_map_size: &mut usize,
        memory_map: *mut MemoryDescriptor,
        map_key: &mut usize,
        descriptor_size: &mut usize,
        descriptor_version: &mut u32,
    ) -> Status,
    pub _allocate_pool:
        extern "win64" fn(pool_type: MemoryType, size: usize, buffer: &mut *mut u8) -> Status,
    pub _free_pool: extern "win64" fn(buffer: *mut u8) -> Status,

    // Event & Timer Services
    pub _create_event: extern "win64" fn(
        event_type: EventType,
        notify_tpl: TaskPriorityLevel,
        notify_function: extern "win64" fn(event: &Event, context: *const ()),
        notify_context: *const (),
        event: &mut &Event,
    ) -> Status,
    pub _set_timer:
        extern "win64" fn(event: &Event, timer_type: TimerDelay, trigger_time: u64) -> Status,
    pub _wait_for_event: extern "win64" fn(
        number_of_events: usize,
        event: *const &Event,
        index: &mut usize,
    ) -> Status,
    pub _signal_event: extern "win64" fn(event: &Event) -> Status,
    pub _close_event: extern "win64" fn(event: &Event) -> Status,
    pub _check_event: extern "win64" fn(event: &Event) -> Status,

    // Protocol Handler Services
    pub _install_protocol_interface: extern "win64" fn(),
    pub _reinstall_protocol_interface: extern "win64" fn(),
    pub _uninstall_protocol_interface: extern "win64" fn(),
    pub _handle_protocol: extern "win64" fn(),
    reserved: AtomicPtr<()>,
    pub _register_protocol_notify: extern "win64" fn(),
    pub _locate_handle: extern "win64" fn(
        search_type: SearchType,
        protocol: *const Guid,
        search_key: *const (),
        buffer_size: &mut usize,
        buffer: *mut Handle,
    ) -> Status,
    pub _locate_device_path: extern "win64" fn(),
    pub _install_configuration_table: extern "win64" fn(),

    // Image Services
    pub _load_image: extern "win64" fn(),
    pub _start_image: extern "win64" fn(),
    pub _exit: extern "win64" fn(),
    pub _unload_image: extern "win64" fn(),
    pub _exit_boot_services: extern "win64" fn(image_handle: Handle, map_key: usize) -> Status,

    // Miscellaneous Services
    pub _get_next_monotonic_count: extern "win64" fn(),
    pub _stall: extern "win64" fn(),
    pub _set_watchdog_timer: extern "win64" fn(),

    // Driver Support Services
    pub _connect_controller: extern "win64" fn(),
    pub _disconnect_controller: extern "win64" fn(),

    // Open and Close Protocol Services
    pub _open_protocol: extern "win64" fn(
        handle: Handle,
        protocol: &Guid,
        interface: &mut *mut (),
        agent_handle: Handle,
        controller_handle: Handle,
        attributes: OpenProtocolAttributes,
    ) -> Status,
    pub _close_protocol: extern "win64" fn(
        handle: Handle,
        protocol: &Guid,
        agent_handle: Handle,
        controller_handle: Handle,
    ) -> Status,
    pub _open_protocol_information: extern "win64" fn(),

    // Library Services
    pub _protocols_per_handle: extern "win64" fn(),
    pub _locate_handle_buffer: extern "win64" fn(),
    pub _locate_protocol: extern "win64" fn(),
    pub _install_multiple_protocol_interfaces: extern "win64" fn(),
    pub _uninstall_multiple_protocol_interfaces: extern "win64" fn(),

    // 32-bit CRC Services
    pub _calculate_crc32: extern "win64" fn(),

    // Miscellaneous Services
    pub _copy_mem: extern "win64" fn(),
    pub _set_mem: extern "win64" fn(buffer: *mut u8, size: usize, value: u8),
    pub _create_event_ex: extern "win64" fn(),
}

impl BootServices {
    pub fn exit_boot_services(&self, image_handle: Handle, map_key: usize) -> Result<(), Status> {
        (self._exit_boot_services)(image_handle, map_key).as_result().map(|_| ())
    }
}

impl fmt::Debug for BootServices {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("BootServices").field("hdr", &self.hdr).finish()
    }
}

/// Encodes the given str to UTF-16 code units
pub fn str_to_utf16(src: &str) -> Result<Pool<[Char16]>, Status> {
    // Allocate a slice of Char16 from pool memory
    // TODO: use boot_services.allocate_slice
    // This needs to be done manually because a slice is not Sized
    let mut buf_len: usize = src
        .chars()
        // 2 bytes per UTF-16 unit
        .map(|c| c.len_utf16() * 2)
        .sum();
    // An extra 2 bytes for a null terminator
    buf_len += 2;
    let mut buf = unsafe {
        let ptr = crate::uefi::system_table()
            .boot_services
            .allocate_pool(MemoryType::LoaderData, buf_len)?;
        Pool::new_unchecked(slice::from_raw_parts_mut(ptr as *mut Char16, buf_len / 2))
    };

    // Copy encoded characters into the new slice
    let mut temp_buf = [0u16; 2];
    let mut current_index = 0;
    for c in src.chars() {
        let units = c.encode_utf16(&mut temp_buf);
        for i in 0..units.len() {
            buf[current_index] = units[i];
            current_index += 1;
        }
    }

    // Add a null terminator
    buf[current_index] = 0u16;

    Ok(buf)
}

/// Decodes a str from the given UTF-16 code units
pub fn utf16_to_str(src: &[Char16]) -> Result<Pool<str>, Status> {
    /*
     * Create an iterator of Rust `char` over the UTF-16 slice, stopping when we encounter a null
     * code-unit.
     */
    let chars = decode_utf16(src.iter().cloned().take_while(|c| *c != 0x0000))
        .map(|r| r.unwrap_or(REPLACEMENT_CHARACTER));

    // Allocate a buffer large enough to hold the string when converted into UTF-8 code units
    let buf_len: usize = chars.clone().map(|c| c.len_utf8()).sum();
    let buf: &mut [u8] = unsafe {
        let ptr = crate::uefi::system_table()
            .boot_services
            .allocate_pool(MemoryType::LoaderData, buf_len)?;
        slice::from_raw_parts_mut(ptr, buf_len)
    };

    // Iterate over the old string, placing the re-encoded bytes into the new buffer
    let mut temp_buf = [0u8; 4];
    let mut current_index = 0;
    for c in chars {
        let bytes = c.encode_utf8(&mut temp_buf).bytes();
        for b in bytes {
            buf[current_index] = b;
            current_index += 1;
        }
    }

    // Re-interpret the buffer as a str behind a custom pointer
    unsafe { Ok(Pool::new_unchecked(from_utf8_unchecked_mut(buf))) }
}
