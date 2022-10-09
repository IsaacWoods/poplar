/*
 * Copyright 2018, The pin-utils authors
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MIT OR Apache-2.0
 */

//! This module includes some macros for more easily working with pinned types. It takes inspiration from the
//! `pin-utils` crate, but extends it to provide custom visibility on the created methods.

/// A pinned projection of a struct field.
///
/// To make using this macro safe, three things need to be ensured:
/// - If the struct implements [`Drop`], the [`drop`] method is not allowed to move the value of the field.
/// - If the struct wants to implement [`Unpin`], it has to do so conditionally: The struct can only implement
///   [`Unpin`] if the field's type is [`Unpin`].
/// - The struct must not be `#[repr(packed)]`.
///
/// ```
/// use poplar_util::unsafe_pinned;
/// use core::marker::Unpin;
/// use core::pin::Pin;
///
/// struct Foo<T> {
///     field: T,
/// }
///
/// impl<T> Foo<T> {
///     unsafe_pinned!(field: T);
///
///     fn baz(mut self: Pin<&mut Self>) {
///         let _: Pin<&mut T> = self.field();
///     }
/// }
///
/// impl<T: Unpin> Unpin for Foo<T> {}
/// ```
///
/// Note that borrowing the field multiple times requires using `.as_mut()` to
/// avoid consuming the `Pin`.
///
/// [`Unpin`]: core::marker::Unpin
/// [`drop`]: Drop::drop
#[macro_export]
macro_rules! unsafe_pinned {
    ($v:vis $f:ident: $t:ty) => {
        #[allow(unsafe_code)]
        $v fn $f<'__a>(
            self: $crate::core_reexport::pin::Pin<&'__a mut Self>,
        ) -> $crate::core_reexport::pin::Pin<&'__a mut $t> {
            unsafe { $crate::core_reexport::pin::Pin::map_unchecked_mut(self, |x| &mut x.$f) }
        }
    };
}

/// An unpinned projection of a struct field.
///
/// This macro is unsafe because it creates a method that returns a normal
/// non-pin reference to the struct field. It is up to the programmer to ensure
/// that the contained value can be considered not pinned in the current
/// context.
///
/// Note that borrowing the field multiple times requires using `.as_mut()` to
/// avoid consuming the `Pin`.
///
/// ```
/// use poplar_util::unsafe_unpinned;
/// use core::pin::Pin;
///
/// struct Bar;
/// struct Foo {
///     field: Bar,
/// }
///
/// impl Foo {
///     unsafe_unpinned!(field: Bar);
///
///     fn baz(mut self: Pin<&mut Self>) {
///         let _: &mut Bar = self.field();
///     }
/// }
/// ```
#[macro_export]
macro_rules! unsafe_unpinned {
    ($v:vis $f:ident: $t:ty) => {
        #[allow(unsafe_code)]
        $v fn $f<'__a>(self: $crate::core_reexport::pin::Pin<&'__a mut Self>) -> &'__a mut $t {
            unsafe { &mut $crate::core_reexport::pin::Pin::get_unchecked_mut(self).$f }
        }
    };
}
