/*
 * Copyright 2021, The Vanadinite Developers
 * Copyright 2022, Isaac Woods
 * SPDX-License-Identifier: MPL-2.0
 */

#![no_std]

use core::{cell::UnsafeCell, marker::PhantomData, ops::Index};

#[derive(Clone, Copy, Debug)]
pub struct Read;
#[derive(Clone, Copy, Debug)]
pub struct Write;
#[derive(Clone, Copy, Debug)]
pub struct ReadWrite;

#[derive(Debug)]
#[repr(transparent)]
pub struct Volatile<T, Access = ReadWrite>(UnsafeCell<T>, PhantomData<Access>);

unsafe impl<T, A> Send for Volatile<T, A> {}
unsafe impl<T, A> Sync for Volatile<T, A> {}

impl<T> Volatile<T, Read>
where
    T: Copy + 'static,
{
    pub fn read(&self) -> T {
        unsafe { self.0.get().read_volatile() }
    }
}

impl<T> Volatile<T, Write>
where
    T: Copy + 'static,
{
    pub fn write(&self, val: T) {
        unsafe { self.0.get().write_volatile(val) }
    }
}

impl<T> Volatile<T, ReadWrite>
where
    T: Copy + 'static,
{
    pub fn read(&self) -> T {
        unsafe { self.0.get().read_volatile() }
    }

    pub fn write(&self, val: T) {
        unsafe { self.0.get().write_volatile(val) }
    }
}

impl<T, const N: usize> Index<usize> for Volatile<[T; N], Read>
where
    T: Copy,
{
    type Output = Volatile<T>;

    #[allow(clippy::transmute_ptr_to_ptr)]
    fn index(&self, index: usize) -> &Self::Output {
        unsafe { &core::mem::transmute::<_, &[Volatile<T>; N]>(self)[index] }
    }
}

impl<T, const N: usize> Index<usize> for Volatile<[T; N], ReadWrite>
where
    T: Copy,
{
    type Output = Volatile<T>;

    #[allow(clippy::transmute_ptr_to_ptr)]
    fn index(&self, index: usize) -> &Self::Output {
        unsafe { &core::mem::transmute::<_, &[Volatile<T>; N]>(self)[index] }
    }
}
