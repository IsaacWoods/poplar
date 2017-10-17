/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

mod area_frame_allocator;
mod paging;
mod stack_allocator;

pub use self::area_frame_allocator::AreaFrameAllocator;
pub use self::paging::remap_kernel;

use self::stack_allocator::{Stack,StackAllocator};
use self::paging::{PAGE_SIZE,PhysicalAddress};
use hole_tracking_allocator::ALLOCATOR;
use multiboot2::BootInformation;

/*
 * TODO: can we share KERNEL_VMA as an external symbol and declare it in the linker script?
 */
pub const KERNEL_VMA : usize = 0xffffffff80000000;
pub const HEAP_START : usize = 0o000_001_000_000_0000;
pub const HEAP_SIZE  : usize = 100 * 1024;  // 100 KiB

pub fn init(boot_info : &BootInformation) -> MemoryController<AreaFrameAllocator>
{
    assert_first_call!("memory::init() should only be called once");

    use x86_64::registers::msr::{IA32_EFER,rdmsr,wrmsr};
    use x86_64::registers::control_regs::{Cr0,cr0,cr0_write};
    use self::paging::Page;

    let memory_map_tag = boot_info.memory_map_tag().expect("Can't find memory map tag");
    let elf_sections_tag = boot_info.elf_sections_tag().expect("Can't find elf sections tag");

    let kernel_start = elf_sections_tag.sections().filter(|s| s.is_allocated()).map(|s| s.addr).min().unwrap();
    let kernel_end = elf_sections_tag.sections().filter(|s| s.is_allocated()).map(|s| s.addr + s.size).max().unwrap();

    println!("Loading kernel to: {:#x}", kernel_start);

    let mut frame_allocator = AreaFrameAllocator::new(boot_info.start_address() as usize,
                                                      boot_info.end_address()   as usize,
                                                      kernel_start              as usize,
                                                      kernel_end                as usize,
                                                      memory_map_tag.memory_areas());

    unsafe
    {
        /*
         * Set the NXE bit in the EFER allows us to use the No-Execute bit on page table entries.
         */
        let efer = rdmsr(IA32_EFER);
        wrmsr(IA32_EFER, efer | (1<<11));

        /*
         * Enable Write Protection
         */
        cr0_write(cr0() | Cr0::WRITE_PROTECT);
    }

    let mut active_table = paging::remap_kernel(&mut frame_allocator, boot_info);

    // Map the pages used by the heap
    let heap_start_page = Page::get_containing_page(HEAP_START);
    let heap_end_page = Page::get_containing_page(HEAP_START + HEAP_SIZE - 1);

    for page in Page::range_inclusive(heap_start_page, heap_end_page)
    {
        active_table.map(page, paging::WRITABLE, &mut frame_allocator);
    }

    // Create the heap
    unsafe
    {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    // Create a StackAllocator that allocates in the 100 pages directly following the heap
    let stack_allocator = StackAllocator::new(Page::range_inclusive(heap_end_page + 1,
                                                                    heap_end_page + 101));

    MemoryController
    {
        active_table    : active_table,
        frame_allocator : frame_allocator,
        stack_allocator : stack_allocator
    }
}

struct FrameIter
{
    start : Frame,
    end   : Frame
}

impl Iterator for FrameIter
{
    type Item = Frame;

    fn next(&mut self) -> Option<Frame>
    {
        if self.start <= self.end
        {
            let frame = self.start.clone();
            self.start.number += 1;
            Some(frame)
        }
        else
        {
            None
        }
    }
}

#[derive(Debug,PartialEq,Eq,PartialOrd,Ord)]
pub struct Frame
{
    number : usize
}

impl Frame
{
    fn get_containing_frame(address : usize) -> Frame
    {
        Frame { number : address / PAGE_SIZE }
    }

    fn get_start_address(&self) -> PhysicalAddress
    {
        self.number * PAGE_SIZE
    }

    fn clone(&self) -> Frame
    {
        Frame { number : self.number }
    }

    fn range_inclusive(start : Frame, end : Frame) -> FrameIter
    {
        FrameIter
        {
            start : start,
            end   : end
        }
    }
}

pub trait FrameAllocator
{
    fn allocate_frame(&mut self) -> Option<Frame>;
    fn deallocate_frame(&mut self, frame : Frame);
}

pub struct MemoryController<A : FrameAllocator>
{
    active_table    : paging::ActivePageTable,
    frame_allocator : A,
    stack_allocator : StackAllocator
}

impl<A> MemoryController<A> where A : FrameAllocator
{
    pub fn alloc_stack(&mut self, size_in_pages : usize) -> Option<Stack>
    {
        let &mut MemoryController
                 {
                     ref mut active_table,
                     ref mut frame_allocator,
                     ref mut stack_allocator
                 } = self;
        stack_allocator.alloc_stack(active_table, frame_allocator, size_in_pages)
    }
}
