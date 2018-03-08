/*
 * Copyright (C) 2018, Isaac Woods.
 * See LICENCE.md
 */

use pebble_syscall_common::{SyscallInfo,SyscallType,SyscallResult};

/*
 * Slices in Rust are represented by two words (where word-size is the same as usize):
 *     -------------------
 *     | length |   ptr  |
 *     -------------------
 *  NOTE: Length is the number of *elements*, not bytes!
 */
unsafe fn as_slice<'a, T>(ptr : usize, length : usize) -> &'a [T]
{
    ::core::slice::from_raw_parts(ptr as *const T, length)
}

pub fn dispatch_syscall(info : &SyscallInfo) -> SyscallResult
{
    info!("{},{},{},{},{},{}", info.syscall_number, info.a, info.b, info.c, info.d, info.e);

    match SyscallType::from(info.syscall_number)
    {
        SyscallType::Unknown => SyscallResult::ErrUnknownSyscall,

        SyscallType::DebugMsg =>
        {
            let msg = ::core::str::from_utf8(unsafe { as_slice(info.a, info.b) }).unwrap();
            trace!("Usermode process says: {}", msg);
            SyscallResult::Success
        },

        SyscallType::Write =>
        {
            SyscallResult::ErrUnknownSyscall
        },
    }
}
