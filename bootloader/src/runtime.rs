use crate::TableHeader;
use crate::uefi::{UefiStatus, Guid};
use crate::memory::MemoryDescriptor;

#[derive(Clone, Copy)]
#[repr(C)]
pub enum ResetType {
    Cold,
    Warm,
    Shutdown,
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct Time {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    _reserved0: u8,
    pub nanosecond: u32,
    pub time_zone: u16,
    pub daylight: u8,
    _reserved1: u8,
}

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
pub struct TimeCapabilities {
  pub resolution: u32,
  pub accuracy: u32,
  pub sets_to_zero: bool,
}

pub const CAPSULE_FLAGS_PERSIST_ACROSS_RESET: u32 = 0x00010000;
pub const CAPSULE_FLAGS_POPULATE_SYSTEM_TABLE: u32 = 0x00020000;
pub const CAPSULE_FLAGS_INITIATE_RESET: u32 = 0x00040000;

#[repr(C)]
pub struct CapsuleHeader {
    pub capsule_guid: Guid,
    pub header_size: u32,
    pub flags: u32,
    pub image_size: u32,
}

#[repr(C)]
pub struct CapsuleBlockDescriptor {
    pub length: u64,
    pub data_block_address: usize,
}

#[repr(C)]
pub struct RuntimeServices {
    pub header: TableHeader,
    pub get_time: extern "win64" fn(time: &mut Time, capabilities: *mut TimeCapabilities) -> UefiStatus,
    pub set_time: extern "win64" fn(time: &Time) -> UefiStatus,
    pub get_wakeup_time: extern "win64" fn(enabled: &mut bool, pending: &mut bool, time: &mut Time) -> UefiStatus,
    pub set_wakeup_time: extern "win64" fn(enabled: bool, time: &mut Time) -> UefiStatus,
    pub set_virtual_address_map: extern "win64" fn(memory_map_size: usize, descriptor_size: usize, descriptor_version: u32, map: *const MemoryDescriptor) -> UefiStatus,
    pub convert_pointer: extern "win64" fn(debug_disposition: usize, address: &mut usize) -> UefiStatus,
    pub get_variable: extern "win64" fn(name: *const u16, vendor_guid: &Guid, attributes: *mut u32, data_size: &mut usize, data: *mut u8) -> UefiStatus,
    pub get_next_variable_name: extern "win64" fn(name_size: &mut usize, name: *mut u16, vendor_guid: &mut Guid) -> UefiStatus,
    pub set_variable: extern "win64" fn(name: *const u16, vendor_guid: &Guid, attributes: u32, data_size: usize, data: *const u8) -> UefiStatus,
    pub get_next_high_monotonic_count: extern "win64" fn(high_count: &mut u32) -> UefiStatus,
    pub reset_system: extern "win64" fn(rest_type: ResetType, reset_status: UefiStatus, data_size: usize, data: *const u8) -> !,
    pub update_capsule: extern "win64" fn(header_array: *const *const CapsuleHeader, count: usize, scatter_gather_list: usize) -> UefiStatus,
    pub query_capsule_capabilities: extern "win64" fn(header_array: *const *const CapsuleHeader, count: usize, maximum_size: &mut u64, reset_type: &mut ResetType) -> UefiStatus,
    pub query_variable_info: extern "win64" fn(attributes: u32, maximum_storage_size: &mut u64, remaining_storage_size: &mut u64, maximum_variable_size: &mut u64) -> UefiStatus,
}
