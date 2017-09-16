/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 *
 * This should only be used before we have a heap. It acts like a Vec but allocates directly into a
 * TemporaryPage.
 */

use super::{PAGE_SIZE,TemporaryPage};
use core::mem::size_of;
use core::ptr::write;
use core::marker::PhantomData;

pub struct TemporaryVec<T>
{
    head        : *mut T,
    tail        : *mut T,
    length      : usize,
    capacity    : usize,
}

impl<T> TemporaryVec<T>
{
    /*
     * Marked unsafe because it is assumed that the TemporaryPage passed is mapped appropriately.
     * Other operations on this are marked safe with the onus on the creator to ensure safety.
     * It should be the sole user of the space allocated to it (this can't be enforced very well).
     *
     * XXX: Offset and capacity are specified in bytes
     */
    pub unsafe fn new(page : &mut TemporaryPage) -> TemporaryVec<T>
    {
        let start_address = page.get_start_address() as *mut T;

        TemporaryVec
        {
            head        : start_address,
            tail        : start_address,
            length      : 0,
            capacity    : PAGE_SIZE / size_of::<T>(),
        }
    }

    pub fn push(&mut self, value : T)
    {
        assert!(self.length < self.capacity);

        unsafe
        {
            write(self.tail, value);
            self.tail = self.tail.offset(1);
        }

        self.length += 1;
    }

    pub fn iter(&self) -> TemporaryVecIter<T>
    {
        TemporaryVecIter
        {
            ptr         : self.head,
            i           : 0,
            length      : self.length,
            _phantom    : PhantomData,
        }
    }
}

pub struct TemporaryVecIter<'a,T : 'a>
{
    ptr         : *mut T,
    i           : usize,
    length      : usize,
    _phantom    : PhantomData<&'a ()>,
}

impl<'a,T> Iterator for TemporaryVecIter<'a,T>
{
    type Item = &'a mut T;

    fn next(&mut self) -> Option<&'a mut T>
    {
        if self.i < self.length
        {
            let value : &'a mut T = unsafe { &mut *self.ptr };
            unsafe { self.ptr = self.ptr.offset(1); }
            self.i += 1;
            Some(value)
        }
        else
        {
            None
        }
    }
}
