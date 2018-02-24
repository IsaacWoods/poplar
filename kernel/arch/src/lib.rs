/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

#![no_std]

#![feature(const_fn)]

#[macro_export]
macro_rules! assert_first_call
{
    () =>
    {
        assert_first_call!("ASSERTION FAILED: function has already been called");
    };

    ($($arg:tt)+) =>
    {{
        fn assert_first_call()
        {
            use core::sync::atomic::{AtomicBool,
                                     ATOMIC_BOOL_INIT,
                                     Ordering};

            static CALLED : AtomicBool = ATOMIC_BOOL_INIT;
            let called = CALLED.swap(true, Ordering::Relaxed);
            assert!(!called, $($arg)+);
        }
        assert_first_call();
    }};
}

/*
 * This trait is implemented by a type in each architecture crate. It provides a common interface
 * to platform-specific operations and types for the rest of the kernel to use.
 */
pub trait Architecture
{
    type MemoryAddress;

    fn clear_screen(&self);
}
