/*
 * Copyright (C) 2017, Isaac Woods.
 * See LICENCE.md
 */

use super::{PAGE_SIZE,FrameAllocator};
use super::paging::{self,Page,PageIter,VirtualAddress,ActivePageTable};

#[derive(Debug)]
pub struct Stack
{
    top     : VirtualAddress,
    bottom  : VirtualAddress,
}

impl Stack
{
    fn new(top : VirtualAddress, bottom : VirtualAddress) -> Stack
    {
        assert!(top > bottom);
        Stack
        {
            top     : top,
            bottom  : bottom
        }
    }

    pub fn top(&self) -> VirtualAddress
    {
        self.top
    }
}

pub struct StackAllocator
{
    range : PageIter
}

impl StackAllocator
{
    pub fn new(space_top : VirtualAddress, space_bottom : VirtualAddress) -> StackAllocator
    {
        StackAllocator
        {
            range : Page::range_inclusive(Page::get_containing_page(space_top),
                                          Page::get_containing_page(space_bottom)),
        }
    }

    pub fn alloc_stack<A : FrameAllocator>(&mut self,
                                           active_table     : &mut ActivePageTable,
                                           frame_allocator  : &mut A,
                                           size_in_pages    : usize) -> Option<Stack>
    {
        if size_in_pages == 0
        {
            return None;
        }

        /*
         * We should only change the range if we successfully create a new stack
         */
        let mut range = self.range.clone();

        let guard_page = range.next();
        let stack_start = range.next();
        let stack_end = if size_in_pages == 1
                        {
                            stack_start
                        }
                        else
                        {
                            range.nth(size_in_pages - 2)
                        };

        match (guard_page,stack_start,stack_end)
        {
            (Some(_),Some(start),Some(end)) =>
            {
                self.range = range;

                for page in Page::range_inclusive(start, end)
                {
                    active_table.map(page, paging::entry::EntryFlags::WRITABLE, frame_allocator);
                }

                let top_of_stack = end.start_address().offset(PAGE_SIZE as isize);
                Some(Stack::new(top_of_stack, start.start_address()))
            }

            _ => None
        }
    }
}
