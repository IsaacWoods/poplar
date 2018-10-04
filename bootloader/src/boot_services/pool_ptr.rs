use core::{
    cmp::PartialEq,
    fmt,
    fmt::{Debug, Display, Formatter, Pointer},
    iter::Iterator,
    ops::{Deref, DerefMut, Drop},
    ptr::Unique,
};

/// A pointer type for UEFI boot services pool allocation
pub struct Pool<T>
where
    T: ?Sized,
{
    ptr: Unique<T>,
}

impl<T: ?Sized> Pool<T> {
    /// Creates a new `Pool`
    ///
    /// # Safety
    ///
    /// `ptr` must be non-null
    pub(crate) unsafe fn new_unchecked(ptr: *mut T) -> Pool<T> {
        Pool {
            ptr: Unique::new_unchecked(ptr),
        }
    }
}

impl<T: ?Sized + Debug> Debug for Pool<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized> Deref for Pool<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { self.ptr.as_ref() }
    }
}

impl<T: ?Sized> DerefMut for Pool<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.ptr.as_mut() }
    }
}

impl<T: ?Sized + Display> Display for Pool<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(&**self, f)
    }
}

impl<T: ?Sized> Drop for Pool<T> {
    fn drop(&mut self) {
        crate::system_table()
            .boot_services
            .free_pool(self.ptr.as_ptr() as *mut u8)
            .expect("failed to deallocate Pool");
    }
}

impl<T: ?Sized + Iterator> Iterator for Pool<T> {
    type Item = T::Item;

    fn next(&mut self) -> Option<T::Item> {
        (**self).next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (**self).size_hint()
    }

    fn nth(&mut self, n: usize) -> Option<T::Item> {
        (**self).nth(n)
    }
}

impl<'a, T: ?Sized + PartialEq> PartialEq<&'a T> for Pool<T> {
    fn eq(&self, other: &&'a T) -> bool {
        PartialEq::eq(&**self, &**other)
    }

    fn ne(&self, other: &&'a T) -> bool {
        PartialEq::ne(&**self, &**other)
    }
}

impl<T: ?Sized> Pointer for Pool<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Pointer::fmt(&self.ptr, f)
    }
}
