/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

pub type MemoryAddress = usize;

/*
 * This trait is implemented by a type in each architecture crate. It provides a common interface
 * to platform-specific operations and types for the rest of the kernel to use.
 */
pub trait Architecture
{
    fn clear_screen(&self);
    fn get_module_address(&self, module_name : &str) -> Option<(MemoryAddress,MemoryAddress)>;
}
