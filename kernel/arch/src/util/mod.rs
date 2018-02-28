/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

pub use self::binary_pretty_print::BinaryPrettyPrint;

pub mod binary_pretty_print;

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
