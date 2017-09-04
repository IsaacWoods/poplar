/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

mod area_frame_allocator;
mod paging;

pub use self::area_frame_allocator::AreaFrameAllocator;
pub use self::paging::remap_kernel;

use self::paging::{PAGE_SIZE,PhysicalAddress};
use multiboot2::BootInformation;

pub fn init(boot_info : &BootInformation)
{
    assert_first_call!("memory::init() should only be called once");

    use x86_64::registers::msr::{IA32_EFER,rdmsr,wrmsr};
    use x86_64::registers::control_regs::{Cr0,cr0,cr0_write};
    use self::paging::Page;
    use bump_allocator::{HEAP_START,HEAP_SIZE};

    let memory_map_tag = boot_info.memory_map_tag().expect("Can't find memory map tag");
    let elf_sections_tag = boot_info.elf_sections_tag().expect("Can't find elf sections tag");

    let kernel_start = elf_sections_tag.sections().filter(|s| s.is_allocated()).map(|s| s.addr).min().unwrap();
    let kernel_end = elf_sections_tag.sections().filter(|s| s.is_allocated()).map(|s| s.addr + s.size).max().unwrap();

    println!("Kernel start: {:#x}, end: {:#x}", kernel_start, kernel_end);
    println!("Multiboot start: {:#x}, end: {:#x}", boot_info.start_address(), boot_info.end_address());


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

    /*
     * Map the pages used by the heap
     */
    let heap_start_page = Page::get_containing_page(HEAP_START);
    let heap_end_page = Page::get_containing_page(HEAP_START + HEAP_SIZE - 1);

    for page in Page::range_inclusive(heap_start_page, heap_end_page)
    {
        active_table.map(page, paging::WRITABLE, &mut frame_allocator);
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
