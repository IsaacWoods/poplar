use super::BootServices;
use core::{
    cmp::PartialEq,
    fmt,
    fmt::{Debug, Display, Formatter, Pointer},
    iter::Iterator,
    ops::{Deref, DerefMut, Drop},
    ptr::Unique,
};

/// A pointer type for UEFI boot services pool allocation
pub struct Pool<'a, T>
where
    T: ?Sized,
{
    ptr: Unique<T>,
    boot_services: &'a BootServices,
}

impl<'a, T: ?Sized> Pool<'a, T> {
    /// Creates a new `Pool`
    ///
    /// # Safety
    ///
    /// `ptr` must be non-null
    pub(crate) unsafe fn new_unchecked(
        ptr: *mut T,
        boot_services: &'a BootServices,
    ) -> Pool<'a, T> {
        Pool {
            ptr: Unique::new_unchecked(ptr),
            boot_services: boot_services,
        }
    }
}

impl<'a, T: ?Sized + Debug> Debug for Pool<'a, T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Debug::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized> Deref for Pool<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { self.ptr.as_ref() }
    }
}

impl<'a, T: ?Sized> DerefMut for Pool<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.ptr.as_mut() }
    }
}

impl<'a, T: ?Sized + Display> Display for Pool<'a, T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized> Drop for Pool<'a, T> {
    fn drop(&mut self) {
        self.boot_services
            .free_pool(self.ptr.as_ptr() as *mut u8)
            .expect("failed to deallocate Pool");
    }
}

impl<'a, T: ?Sized + Iterator> Iterator for Pool<'a, T> {
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

impl<'a, 'b, T: ?Sized + PartialEq> PartialEq<&'b T> for Pool<'a, T> {
    fn eq(&self, other: &&'b T) -> bool {
        PartialEq::eq(&**self, &**other)
    }

    fn ne(&self, other: &&'b T) -> bool {
        PartialEq::ne(&**self, &**other)
    }
}

impl<'a, T: ?Sized> Pointer for Pool<'a, T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Pointer::fmt(&self.ptr, f)
    }
}
