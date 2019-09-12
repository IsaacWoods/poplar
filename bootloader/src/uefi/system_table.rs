use crate::uefi::{
    boot_services::BootServices,
    protocols::ConsoleOut,
    runtime_services::RuntimeServices,
    Guid,
    Handle,
    RuntimeMemory,
    TableHeader,
};
use core::slice;

/// The UEFI system table describes the services the UEFI provides to the bootloader. We don't
/// support the console services, because they can allocate at any point and so are difficult to
/// use safely.
// TODO: why do all of these use `RuntimeMemory` (even the boot-services-only stuff?)
#[repr(C)]
pub struct SystemTable {
    pub hdr: TableHeader,
    pub firmware_vendor: RuntimeMemory<u16>,
    pub firmware_revision: u32,
    pub _console_in_handle: Handle,
    pub _console_in: RuntimeMemory<()>,
    pub _console_out_handle: Handle,
    pub console_out: RuntimeMemory<ConsoleOut>,
    pub _standard_error_handle: Handle,
    pub _console_error: RuntimeMemory<()>,
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

#[derive(Debug)]
#[repr(C)]
pub struct ConfigTableEntry {
    pub guid: Guid,
    pub address: usize,
}
