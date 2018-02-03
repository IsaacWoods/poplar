/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use core::fmt;
use core::ops::{Add,Sub};
use core::cmp::Ordering;
use super::PAGE_SIZE;

#[derive(Clone,Copy,Debug)]
pub struct VirtualAddress(pub(super) usize);

impl VirtualAddress
{
    pub const fn new(address : usize) -> VirtualAddress
    {
        VirtualAddress(address)
    }

    pub const fn from_page_table_offsets(p4     : usize,
                                         p3     : usize,
                                         p2     : usize,
                                         p1     : usize,
                                         offset : usize) -> VirtualAddress
    {
        VirtualAddress::new((p4<<39) |
                            (p3<<30) |
                            (p2<<21) |
                            (p1<<12) |
                            (offset<<0)).canonicalise()
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

    /*
     * Addresses are always expected by the CPU to be canonical (bits 48 to 63 are the same as bit
     * 47). If a calculation leaves an address non-canonical, make sure to re-canonicalise it with
     * this function.
     */
    pub const fn canonicalise(self) -> VirtualAddress
    {
        VirtualAddress::new(0o177777_000_000_000_000_0000 * ((self.0 >> 47) & 0b1) |
                            (self.0 & ((1 << 48) - 1)))
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
