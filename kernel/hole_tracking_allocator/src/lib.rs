/*
 * Copyright (C) 2017, Pebble Developers.
 * See LICENCE.md
 */

#![no_std]

#![feature(const_fn)]
#![feature(alloc)]
#![feature(allocator_api)]
#![feature(global_allocator)]
#![feature(pointer_methods)]

#[macro_use] extern crate kernel;
             extern crate alloc;
             extern crate spin;
#[macro_use] extern crate log;

mod hole;

use core::mem;
use core::cmp::max;
use core::ops::Deref;
use core::alloc::Opaque;
use alloc::allocator::{GlobalAlloc,Layout};
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
        assert_first_call!("HoleAllocator::init() must only be called once");

        self.heap_bottom = heap_bottom;
        self.heap_size   = heap_size;
        self.holes       = HoleList::new(heap_bottom, heap_size);
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

unsafe impl GlobalAlloc for LockedHoleAllocator
{
    unsafe fn alloc(&self, layout : Layout) -> *mut Opaque
    {
        let size = max(layout.size(), HoleList::get_min_size());
        let size = align_up(size, mem::align_of::<Hole>());
        let layout = Layout::from_size_align(size, layout.align()).unwrap();

        self.0.lock().holes.allocate_first_fit(layout).unwrap_or(0x0 as *mut Opaque)
    }

    unsafe fn dealloc(&self, ptr : *mut Opaque, layout : Layout)
    {
        let size = max(layout.size(), HoleList::get_min_size());
        let size = align_up(size, mem::align_of::<Hole>());
        let layout = Layout::from_size_align(size, layout.align()).unwrap();

        self.0.lock().holes.deallocate(ptr, layout)
    }
}

/*
 * Get the greatest x with the given alignment such that x <= the given address. The alignment
 * must be a power of two.
 */
pub fn align_down(addr : usize, align : usize) -> usize
{
    assert!(align == 0 || align.is_power_of_two(), "Can only align to a power of two");

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
    else
    {
        assert!(align == 0);
        addr
    }
}

/*
 * Get the smallest x with the given alignment such that x >= the given address.
 */
pub fn align_up(addr : usize, align : usize) -> usize
{
    align_down(addr + align - 1, align)
}
