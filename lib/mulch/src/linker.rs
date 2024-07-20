/*
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

extern "C" {
    /// `LinkerSymbol` is an extern type that represents a symbol defined by the linker. It is entirely opaque to
    /// the Rust type system, and cannot be instantiated, which aims to avoid mistakes where it is taken by value
    /// instead of by reference.
    ///
    /// Symbols can be defined with something like, and accessed only via the `LinkerSymbol::ptr` method:
    /// ```ignore
    /// extern "C" {
    ///     static _stack_top: LinkerSymbol;
    /// }
    /// let stack_top: *const u8 = _stack_top.ptr();
    /// ```
    pub type LinkerSymbol;
}

impl LinkerSymbol {
    pub fn ptr(&'static self) -> *const u8 {
        self as *const Self as *const u8
    }
}

unsafe impl Send for LinkerSymbol {}
unsafe impl Sync for LinkerSymbol {}
