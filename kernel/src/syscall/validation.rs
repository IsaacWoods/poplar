//! This module contains functions that help us validate the inputs to system calls that try to
//! make sure userspace can't crash or exploit the kernel in any way. For example, if we take an
//! address from userspace, we should make sure it's mapped (so we don't page-fault) and an address
//! that userspace could ordinarily access itself (otherwise, we could leak information to a
//! userspace task that it shouldn't be able to access).

use core::{marker::PhantomData, slice, str};

pub struct UserPointer<T>(*mut T);

impl<T> UserPointer<T> {
    pub fn read(&self) -> Result<T, ()> {
        unimplemented!()
    }

    pub fn write(&mut self, value: T) -> Result<(), ()> {
        unimplemented!()
    }

    fn validate_access(&self, needs_write: bool) -> bool {
        // TODO: make sure:
        //   - this is actually a user-space address, and we aren't reading from / writing to a part of the kernel
        //   - it's actually mapped (we aren't going to page fault)
        //   - if we're writing, that it's a writable address
        unimplemented!()
    }
}

/// Represents a slice of `T`s in userspace.
pub struct UserSlice<'a, T> {
    length: usize,
    ptr: *mut T,
    _phantom: PhantomData<&'a ()>,
}

impl<'a, T> UserSlice<'a, T> {
    pub fn new(length: usize, ptr: *mut T) -> UserSlice<'a, T> {
        UserSlice { length, ptr, _phantom: PhantomData }
    }

    pub fn validate_read(&self) -> Result<&'a [T], ()> {
        // TODO: validate access is valid
        Ok(unsafe { slice::from_raw_parts(self.ptr, self.length) })
    }
}

pub struct UserString<'a>(UserSlice<'a, u8>);

impl<'a> UserString<'a> {
    pub fn new(length: usize, ptr: *mut u8) -> UserString<'a> {
        UserString(UserSlice::new(length, ptr))
    }

    pub fn validate(&self) -> Result<&'a str, ()> {
        str::from_utf8(self.0.validate_read()?).map_err(|_| ())
    }
}
