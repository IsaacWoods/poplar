/*
 * Copyright (C) 2018, Isaac Woods.
 * See LICENCE.md
 */

#[derive(Clone,Copy,Debug)]
#[cfg_attr(target_arch="x86_64", repr(packed))]
pub struct SyscallInfo
{
    syscall_number  : usize,
    a               : usize,
    b               : usize,
    c               : usize,
    d               : usize,
    e               : usize,
}

#[inline(never)]
pub fn dispatch_syscall(info : &SyscallInfo) -> usize
{
    info!("{},{},{},{},{},{}", info.syscall_number, info.a, info.b, info.c, info.d, info.e);
    0xDEADBEEF
    // TODO
}
