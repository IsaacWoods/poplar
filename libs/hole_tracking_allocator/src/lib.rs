/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

#![feature(unique)]
#![feature(const_fn)]
#![feature(alloc,allocator_api)]
#![feature(global_allocator)]
#![no_std]

mod hole;

extern crate alloc;
extern crate spin;

use core::mem;
use core::cmp::max;
use core::ops::Deref;
use alloc::allocator::{Alloc,Layout,AllocErr};
use spin::Mutex;
use hole::{Hole,HoleList};

#[global_allocator]
pub static ALLOCATOR : LockedHoleAllocator = LockedHoleAllocator::empty();

pub struct HoleAllocator
{
    heap_bottom : usize,
    heap_size   : usize,
    holes       : HoleList
}

impl HoleAllocator
{
    const fn empty() -> HoleAllocator
    {
        HoleAllocator
        {
            heap_bottom : 0,
            heap_size   : 0,
            holes       : HoleList::empty()
        }
    }

    /*
     * The range [heap_bottom,heap_buttom+heap_size) must be unused.
     * Unsafe because undefined behaviour may occur if the given address range is invalid.
     * XXX: Must only be called once.
     */
    pub unsafe fn init(&mut self, heap_bottom : usize, heap_size : usize)
    {
        self.heap_bottom = heap_bottom;
        self.heap_size   = heap_size;
        self.holes       = HoleList::new(heap_bottom, heap_size);
    }
}

unsafe impl Alloc for HoleAllocator
{
    unsafe fn alloc(&mut self, layout : Layout) -> Result<*mut u8,AllocErr>
    {
        let size = max(layout.size(), HoleList::get_min_size());
        let size = align_up(size, mem::align_of::<Hole>());
        let layout = Layout::from_size_align(size, layout.align()).unwrap();

        self.holes.allocate_first_fit(layout)
    }

    unsafe fn dealloc(&mut self, ptr : *mut u8, layout : Layout)
    {
        let size = max(layout.size(), HoleList::get_min_size());
        let size = align_up(size, mem::align_of::<Hole>());
        let layout = Layout::from_size_align(size, layout.align()).unwrap();

        self.holes.deallocate(ptr, layout)
    }
}

pub struct LockedHoleAllocator(Mutex<HoleAllocator>);

impl LockedHoleAllocator
{
    const fn empty() -> LockedHoleAllocator
    {
        LockedHoleAllocator(Mutex::new(HoleAllocator::empty()))
    }
}

impl Deref for LockedHoleAllocator
{
    type Target = Mutex<HoleAllocator>;

    fn deref(&self) -> &Mutex<HoleAllocator>
    {
        &self.0
    }
}

unsafe impl<'a> Alloc for &'a LockedHoleAllocator
{
    unsafe fn alloc(&mut self, layout : Layout) -> Result<*mut u8,AllocErr>
    {
        self.0.lock().alloc(layout)
    }

    unsafe fn dealloc(&mut self, ptr : *mut u8, layout : Layout)
    {
        self.0.lock().dealloc(ptr, layout)
    }
}

/*
 * Get the greatest x with the given alignment such that x <= the given address. The alignment
 * must be a power of two.
 */
pub fn align_down(addr : usize, align : usize) -> usize
{
    if align.is_power_of_two()
    {
        /*
         * E.g.
         *      align       =   0b00001000
         *      align-1     =   0b00000111
         *      !(align-1)  =   0b11111000
         *                             ^^^ Masks the address to the value below it with the
         *                                 correct alignment
         */
        addr & !(align - 1)
    }
    else if align == 0
    {
        addr
    }
    else
    {
        panic!("Can only align to a power of 2");
    }
}

/*
 * Get the smallest x with the given alignment such that x >= the given address.
 */
pub fn align_up(addr : usize, align : usize) -> usize
{
    align_down(addr + align - 1, align)
}
