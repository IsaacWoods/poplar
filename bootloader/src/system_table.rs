use crate::{
    boot_services::BootServices,
    protocols::{SimpleTextInput, SimpleTextOutput},
    runtime_services::RuntimeServices,
    types::Handle,
    types::{RuntimeMemory, TableHeader},
};

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
    pub number_of_table_entries: usize,
    pub configuration_table: usize, // TODO: abstract over this somehow
}
