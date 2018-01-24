/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use core::fmt;
use core::ops::{Add,Sub};
use core::cmp::Ordering;
use super::PAGE_SIZE;

#[derive(Clone,Copy,Debug)]
pub struct VirtualAddress(usize);

impl VirtualAddress
{
    pub const fn new(address : usize) -> VirtualAddress
    {
        VirtualAddress(address)
    }

    pub const fn ptr<T>(self) -> *const T
    {
        self.0 as *const T
    }

    pub const fn mut_ptr<T>(self) -> *mut T
    {
        self.0 as *mut T
    }

    pub const fn offset(&self, offset : usize) -> VirtualAddress
    {
        VirtualAddress::new(self.0 + offset)
    }

    pub const fn is_page_aligned(&self) -> bool
    {
        self.0 % PAGE_SIZE == 0
    }

    pub const fn offset_into_page(&self) -> usize
    {
        self.0 % PAGE_SIZE
    }
}

impl fmt::LowerHex for VirtualAddress
{
    fn fmt(&self, f : &mut fmt::Formatter) -> fmt::Result
    {
        write!(f, "{:x}", self.0)
    }
}

impl fmt::UpperHex for VirtualAddress
{
    fn fmt(&self, f : &mut fmt::Formatter) -> fmt::Result
    {
        write!(f, "{:x}", self.0)
    }
}

impl From<usize> for VirtualAddress
{
    fn from(address : usize) -> VirtualAddress
    {
        VirtualAddress(address)
    }
}

impl From<VirtualAddress> for usize
{
    fn from(address : VirtualAddress) -> usize
    {
        address.0
    }
}

/*impl Into<usize> for VirtualAddress
{
    fn into(self) -> usize
    {
        self.0
    }
}*/

impl<T> From<*const T> for VirtualAddress
{
    fn from(ptr : *const T) -> VirtualAddress
    {
        (ptr as usize).into()
    }
}

impl Add<VirtualAddress> for VirtualAddress
{
    type Output = VirtualAddress;

    fn add(self, rhs : VirtualAddress) -> VirtualAddress
    {
        (self.0 + rhs.0).into()
    }
}

impl Sub<VirtualAddress> for VirtualAddress
{
    type Output = VirtualAddress;

    fn sub(self, rhs : VirtualAddress) -> VirtualAddress
    {
        (self.0 - rhs.0).into()
    }
}

impl PartialEq<VirtualAddress> for VirtualAddress
{
    fn eq(&self, other : &VirtualAddress) -> bool
    {
        self.0 == other.0
    }
}

impl Eq for VirtualAddress { }

impl PartialOrd<VirtualAddress> for VirtualAddress
{
    fn partial_cmp(&self, rhs : &VirtualAddress) -> Option<Ordering>
    {
        self.0.partial_cmp(&rhs.0)
    }
}

impl Ord for VirtualAddress
{
    fn cmp(&self, rhs : &VirtualAddress) -> Ordering
    {
        self.0.cmp(&rhs.0)
    }
}
