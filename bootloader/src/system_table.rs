use crate::{
    boot_services::BootServices,
    protocols::{SimpleTextInput, SimpleTextOutput},
    runtime_services::RuntimeServices,
    types::{Guid, Handle, RuntimeMemory, TableHeader},
};
use core::slice;

#[derive(Debug)]
#[repr(C)]
pub struct SystemTable {
    pub hdr: TableHeader,
    pub firmware_vendor: RuntimeMemory<u16>,
    pub firmware_revision: u32,
    pub console_in_handle: Handle,
    pub console_in: RuntimeMemory<SimpleTextInput>,
    pub console_out_handle: Handle,
    pub console_out: RuntimeMemory<SimpleTextOutput>,
    pub standard_error_handle: Handle,
    pub console_error: RuntimeMemory<SimpleTextOutput>,
    pub runtime_services: RuntimeMemory<RuntimeServices>,
    pub boot_services: RuntimeMemory<BootServices>,
    pub number_config_entries: usize,
    pub configuration_table: *const ConfigTableEntry,
}

impl SystemTable {
    pub fn config_table(&self) -> &[ConfigTableEntry] {
        unsafe { slice::from_raw_parts(self.configuration_table, self.number_config_entries) }
    }
}

#[repr(C)]
pub struct ConfigTableEntry {
    pub guid: Guid,
    pub address: *const (),
}
