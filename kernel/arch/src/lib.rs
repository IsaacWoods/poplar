/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

#![no_std]

#![feature(const_fn)]

extern crate num_traits;

pub mod util;

/*
 * This trait is implemented by a type in each architecture crate. It provides a common interface
 * to platform-specific operations and types for the rest of the kernel to use.
 */
pub trait Architecture
{
    type MemoryAddress;

    fn clear_screen(&self);
}
