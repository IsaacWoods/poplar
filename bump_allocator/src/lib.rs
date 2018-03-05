/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

#![feature(const_fn)]
#![feature(alloc,allocator_api)]
#![feature(global_allocator)]
#![no_std]

extern crate alloc;
extern crate spin;

use core::ops::Deref;
use alloc::allocator::{Alloc,Layout,AllocErr};
use spin::Mutex;

pub const HEAP_START : usize = 0o000_001_000_000_0000;
pub const HEAP_SIZE  : usize = 100 * 1024;  // 100 KiB

#[global_allocator]
static BUMP_ALLOCATOR : LockedBumpAllocator = LockedBumpAllocator::new(HEAP_START, HEAP_SIZE);

#[derive(Debug)]
struct BumpAllocator
{
    heap_start : usize,
    heap_size  : usize,
    next       : usize,
}

impl BumpAllocator
{
    const fn new(heap_start : usize, heap_size : usize) -> BumpAllocator
    {
        BumpAllocator
        {
            heap_start : heap_start,
            heap_size  : heap_size,
            next       : heap_start
        }
    }
}

unsafe impl Alloc for BumpAllocator
{
    unsafe fn alloc(&mut self, layout : Layout) -> Result<*mut u8,AllocErr>
    {
        let alloc_start = align_up(self.next, layout.align());
        let alloc_end   = alloc_start.saturating_add(layout.size());

        if alloc_end <= self.heap_start + self.heap_size
        {
            self.next = alloc_end;
            Ok(alloc_start as *mut u8)
        }
        else
        {
            Err(AllocErr::Exhausted { request : layout })
        }
    }

    /*
     * This is called if we run out of memory. The default implementation caused an Invalid Opcode
     * Exception, which is unwanted. Instead, we just panic the kernel.
     */
    fn oom(&mut self, _ : AllocErr) -> !
    {
        panic!("Out of memory");
    }

    unsafe fn dealloc(&mut self, _ptr : *mut u8, _layout : Layout)
    {
        // We just leak the memory
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

struct LockedBumpAllocator(Mutex<BumpAllocator>);

impl LockedBumpAllocator
{
    const fn new(heap_start : usize, heap_size : usize) -> LockedBumpAllocator
    {
        LockedBumpAllocator(Mutex::new(BumpAllocator::new(heap_start, heap_size)))
    }
}

impl Deref for LockedBumpAllocator
{
    type Target = Mutex<BumpAllocator>;

    fn deref(&self) -> &Mutex<BumpAllocator>
    {
        &self.0
    }
}

unsafe impl<'a> Alloc for &'a LockedBumpAllocator
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
