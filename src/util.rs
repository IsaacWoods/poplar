/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

#[macro_export]
macro_rules! assert_first_call
{
    () =>
    {
        assert_first_call!("assertion failed: function has already been called");
    };

    ($($arg:tt)+) =>
    {{
        fn assert_first_call()
        {
            use $crate::core::sync::atomic::{AtomicBool,ATOMIC_BOOL_INIT,Ordering};
            static CALLED : AtomicBool = ATOMIC_BOOL_INIT;
            let called = CALLED.swap(true, Ordering::Relaxed);
            assert!(!called, $($arg)+);
        }
        assert_first_call();
    }};
}
