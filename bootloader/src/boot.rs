use crate::memory::{MemoryDescriptor, MemoryType};
use crate::uefi::{Event, Guid, Handle, NotImplemented, UefiStatus};
use crate::TableHeader;

#[repr(C)]
pub enum InterfaceType {
    Native,
}

#[repr(C)]
pub enum LocateSearchType {
    AllHandles,
    ByRegisterNotify,
    ByProtocol,
}

#[repr(C)]
pub struct BootServices {
    pub header: TableHeader,
    pub raise_tpl: extern "win64" fn(new_tpl: usize) -> usize,
    pub restore_tpl: extern "win64" fn(old_tpl: usize),
    pub allocate_pages: extern "win64" fn(
        allocation_type: usize,
        memory_type: MemoryType,
        pages: usize,
        memory: &mut usize,
    ) -> UefiStatus,
    pub free_pages: extern "win64" fn(memory: usize, pages: usize) -> UefiStatus,
    pub get_memory_map: extern "win64" fn(
        memory_map_size: &mut usize,
        memory_map: *mut MemoryDescriptor,
        map_key: &mut usize,
        descriptor_size: &mut usize,
        descriptor_version: &mut u32,
    ) -> UefiStatus,
    pub allocate_pool:
        extern "win64" fn(pool_type: MemoryType, size: usize, buffer: &mut usize) -> UefiStatus,
    pub free_pool: extern "win64" fn(buffer: usize) -> UefiStatus,
    create_event: NotImplemented,
    set_timer: NotImplemented,
    pub wait_for_event:
        extern "win64" fn(num_events: usize, event: *const Event, index: &mut usize) -> UefiStatus,
    signal_event: NotImplemented,
    close_event: NotImplemented,
    check_event: NotImplemented,
    pub install_protocol_interface: extern "win64" fn(
        handle: &mut Handle,
        protocol: &Guid,
        interface_type: InterfaceType,
        interface: usize,
    ) -> UefiStatus,
    reinstall_protocol_interface: NotImplemented,
    pub uninstall_protocol_interface:
        extern "win64" fn(handle: Handle, protocol: &Guid, interface: usize) -> UefiStatus,
    pub handle_protocol:
        extern "win64" fn(handle: Handle, protocol: &Guid, interface: &mut usize) -> UefiStatus,
    _reserved: usize,
    register_protocol_notify: NotImplemented,
    pub locate_handle: extern "win64" fn(
        search_type: LocateSearchType,
        protocol: &Guid,
        search_key: usize,
        buffer_size: &mut usize,
        buffer: *mut Handle,
    ) -> UefiStatus,
    locate_device_path: NotImplemented,
    install_configuration_table: NotImplemented,
    pub load_image: extern "win64" fn(
        boot_policy: bool,
        parent_image_handle: Handle,
        device_path: usize,
        source_buffer: *const u8,
        source_size: usize,
        image_handle: &mut Handle,
    ) -> UefiStatus,
    pub start_image: extern "win64" fn(
        image_handle: Handle,
        exit_data_size: &mut usize,
        exit_data: &mut *mut u16,
    ) -> UefiStatus,
    pub exit: extern "win64" fn(
        image_handle: Handle,
        exit_status: isize,
        exit_data_size: usize,
        exit_data: *const u16,
    ) -> UefiStatus,
    unload_image: NotImplemented,
    pub exit_boot_services: extern "win64" fn(image_handle: Handle, map_key: usize) -> UefiStatus,
    get_next_monotonic_count: NotImplemented,
    pub stall: extern "win64" fn(microseconds: usize) -> UefiStatus,
    pub set_watchdog_timer: extern "win64" fn(
        timeout: usize,
        watchdog_code: u64,
        data_size: usize,
        watchdog_data: *const u16,
    ) -> UefiStatus,
    connect_controller: NotImplemented,
    disconnect_controller: NotImplemented,
    open_protocol: NotImplemented,
    close_protocol: NotImplemented,
    open_protocol_information: NotImplemented,
    pub protocols_per_handle:
        extern "win64" fn(handle: Handle, protocol_buffer: *mut Guid, protocol_buffer_count: usize)
            -> UefiStatus,
    pub locate_handle_buffer: extern "win64" fn(
        search_type: LocateSearchType,
        protocol: &Guid,
        search_key: usize,
        no_handles: &mut usize,
        buffer: &mut *mut Handle,
    ),
    pub locate_protocol:
        extern "win64" fn(protocol: &Guid, registration: usize, interface: &mut usize)
            -> UefiStatus,
    install_multiple_protocol_interfaces: NotImplemented,
    uninstall_multiple_protocol_interfaces: NotImplemented,
    calculate_crc32: NotImplemented,
    copy_mem: NotImplemented,
    set_mem: NotImplemented,
    create_event_ex: NotImplemented,
}
