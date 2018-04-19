/*
 * Copyright (C) 2018, Pebble Developers.
 * See LICENCE.md
 */

use libpebble::syscall::{SyscallInfo,SyscallType,SyscallResult};

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
    trace!("Syscall: {:?}", info);

    match SyscallType::from(info.syscall_number)
    {
        SyscallType::Unknown =>
        {
            warn!("Unknown syscall with number {} issued by process!", unsafe { info.syscall_number });
            SyscallResult::ErrUnknownSyscall
        },

        SyscallType::DebugMsg =>
        {
            let msg = ::core::str::from_utf8(unsafe { as_slice(info.a, info.b) }).unwrap();
            info!("Usermode process says: {}", msg);
            SyscallResult::Success
        },

        SyscallType::Write =>
        {
            SyscallResult::ErrUnknownSyscall
        },
    }
}
