/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use process::ProcessId;
use libpebble::fs::FileHandle;

/// This represents a memory address, whatever that might be on the given architecture. It is
/// always the maximum integer width for the current platform, so should be guaranteed to hold a
/// memory address (on normal architectures, at least).
///
/// On architectures with paging, this may represent a physical or virtual memory address.
pub type MemoryAddress = usize;

/// This represents a module loaded into memory by the bootloader. On architectures with paging,
/// this contains the correct physical and virtual mappings of the module. Otherwise, these
/// addresses are equivelent.
#[derive(Clone,Copy)]
pub struct ModuleMapping
{
    pub physical_start  : MemoryAddress,
    pub physical_end    : MemoryAddress,

    pub virtual_start   : MemoryAddress,
    pub virtual_end     : MemoryAddress,
}

/// This trait is implemented by a type in each architecture crate. It provides a common interface
/// to platform-specific operations and types for the rest of the kernel to use.
pub trait Architecture
{
    fn clear_screen(&self);
    fn get_module_mapping(&self, module_name : &str) -> Option<ModuleMapping>;
    fn create_process(&mut self, file : &FileHandle) -> ProcessId;
}
